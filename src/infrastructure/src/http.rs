//! HTTP adapter: specs `evaluate-http-api`, `redemption` (routes), `promotion-catalog`
//! (routes), `coupon-lifecycle` (generation, grants, grant rules) and `customer-coupons`.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, MatchedPath, Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use discount_application::ports::{
    Clock, EventPublisher, GenerateCoupons, GrantRule, GrantTrigger, PromotionCatalog,
    PromotionMetadata, RedemptionError, RedemptionRepository, ReserveOutcome, StorageError,
};
use discount_application::{
    CancelRedemption, CodeSource, ConfirmRedemption, CouponError, CustomerCouponError, Evaluate,
    EvaluateError, GenerateCouponsUseCase, GrantCoupon, ListCustomerCoupons, Redeem,
    SaveCustomerCoupon,
};
use discount_domain::{
    CartSnapshot, Coupon, CustomerContext, ExchangeRate, Money, PricingPolicy, PricingResult,
    Promotion, StackingGroup,
};
use opentelemetry::global;
use opentelemetry::trace::TraceContextExt;
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::telemetry::{Instruments, evaluate_span};

/// Everything a request handler needs. Ports are trait objects so the binary and the tests
/// can plug different implementations.
pub struct AppState {
    pub catalog: Arc<dyn PromotionCatalog>,
    pub redemptions: Arc<dyn RedemptionRepository>,
    pub clock: Arc<dyn Clock>,
    pub events: Arc<dyn EventPublisher>,
    pub codes: Arc<dyn CodeSource>,
    pub instruments: Instruments,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluateRequest {
    pub cart: CartSnapshot,
    pub customer: CustomerContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedeemRequest {
    pub order_id: String,
    pub cart: CartSnapshot,
    pub customer: CustomerContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetBody {
    pub remaining: Option<Money>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/evaluate", post(evaluate))
        .route("/v1/redeem", post(redeem))
        .route("/v1/redemptions/{order_id}", get(get_redemption))
        .route("/v1/redemptions/{order_id}/confirm", post(confirm))
        .route("/v1/redemptions/{order_id}/cancel", post(cancel))
        .route("/v1/policy", get(get_policy).put(put_policy))
        .route("/v1/groups", get(list_groups))
        .route(
            "/v1/groups/{group_id}",
            get(get_group).put(put_group).delete(delete_group),
        )
        .route("/v1/promotions", get(list_promotions))
        .route("/v1/promotion-metadata", get(list_promotion_metadata))
        .route(
            "/v1/promotions/{promotion_id}/allowlist",
            get(get_allowlist)
                .put(put_allowlist)
                .delete(delete_allowlist),
        )
        .route(
            "/v1/promotions/{promotion_id}/allowlist/customers/{customer_id}",
            get(is_allowlisted)
                .put(add_to_allowlist)
                .delete(remove_from_allowlist),
        )
        .route(
            "/v1/promotions/{promotion_id}/metadata",
            get(get_promotion_metadata)
                .put(put_promotion_metadata)
                .delete(delete_promotion_metadata),
        )
        .route(
            "/v1/promotions/{promotion_id}",
            get(get_promotion)
                .put(put_promotion)
                .delete(delete_promotion),
        )
        .route(
            "/v1/promotions/{promotion_id}/budget",
            get(get_budget).put(put_budget),
        )
        .route("/v1/coupons", get(list_coupons))
        .route("/v1/coupons/generate", post(generate_coupons))
        .route("/v1/coupons/grant", post(grant_coupon))
        .route("/v1/grant-rules", get(list_grant_rules))
        .route(
            "/v1/customers/{customer_id}/coupons",
            get(list_customer_coupons),
        )
        .route(
            "/v1/customers/{customer_id}/coupons/{code}",
            axum::routing::put(save_customer_coupon).delete(forget_customer_coupon),
        )
        .route(
            "/v1/grant-rules/{rule_id}",
            get(get_grant_rule)
                .put(put_grant_rule)
                .delete(delete_grant_rule),
        )
        .route(
            "/v1/coupons/{code}",
            get(get_coupon).put(put_coupon).delete(delete_coupon),
        )
        .route("/v1/exchange-rates", get(list_rates))
        .route(
            "/v1/exchange-rates/{from}/{to}",
            axum::routing::put(put_rate).delete(delete_rate),
        )
        .route("/healthz", get(healthz))
        .route("/openapi.yaml", get(openapi_yaml))
        .route("/docs", get(swagger_ui))
        .layer(trace_layer())
        .with_state(state)
}

/// The HTTP server span (spec `telemetry`, server-span requirements): named by the matched
/// route template, parented to an incoming W3C trace context, carrying `http.route`,
/// `url.path` and the response status; `GET /healthz` gets no span at all. 5xx responses
/// set the span status to `Error`; 4xx (including `422` pricing rejections) do not.
pub fn trace_layer() -> HttpTraceLayer {
    TraceLayer::new_for_http()
        .make_span_with(make_span as MakeSpan)
        .on_response(on_response as OnResponse)
}

type MakeSpan = fn(&Request) -> tracing::Span;
type OnResponse = fn(&Response, Duration, &tracing::Span);
pub type HttpTraceLayer = TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
    MakeSpan,
    tower_http::trace::DefaultOnRequest,
    OnResponse,
>;

fn make_span(req: &Request) -> tracing::Span {
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_owned());
    if route.as_deref() == Some("/healthz") {
        return tracing::Span::none();
    }
    let name = match &route {
        Some(r) => format!("{} {}", req.method(), r),
        None => req.method().to_string(),
    };
    let span = tracing::info_span!(
        "http.request",
        otel.name = %name,
        otel.kind = "server",
        http.request.method = %req.method(),
        http.route = tracing::field::Empty,
        url.path = %req.uri().path(),
        http.response.status_code = tracing::field::Empty,
    );
    if let Some(r) = &route {
        span.record("http.route", r.as_str());
    }
    let parent = global::get_text_map_propagator(|p| p.extract(&HeaderExtractor(req.headers())));
    if parent.span().span_context().is_valid() {
        // Fails only when no OpenTelemetry layer is installed (plain stdout logging).
        let _ = span.set_parent(parent);
    }
    span
}

fn on_response(res: &Response, _latency: Duration, span: &tracing::Span) {
    let status = res.status();
    span.record("http.response.status_code", status.as_u16() as i64);
    if status.is_server_error() {
        span.set_status(opentelemetry::trace::Status::error(
            status
                .canonical_reason()
                .unwrap_or("server error")
                .to_owned(),
        ));
    }
}

/// `opentelemetry::propagation::Extractor` over the request headers.
struct HeaderExtractor<'a>(&'a HeaderMap);

impl opentelemetry::propagation::Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// The OpenAPI document, embedded from `api/openapi.yaml` (the contract, ADR 10).
pub const OPENAPI_YAML: &str = include_str!("../../../api/openapi.yaml");

async fn openapi_yaml() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "application/yaml")],
        OPENAPI_YAML,
    )
        .into_response()
}

const SWAGGER_UI: &str = r##"<!doctype html><html><head><meta charset="utf-8"><title>discount API</title>
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/swagger-ui/5.17.14/swagger-ui.css"></head>
<body><div id="swagger-ui"></div>
<script src="https://cdnjs.cloudflare.com/ajax/libs/swagger-ui/5.17.14/swagger-ui-bundle.js"></script>
<script>window.onload=()=>SwaggerUIBundle({url:"/openapi.yaml",dom_id:"#swagger-ui"});</script></body></html>"##;

async fn swagger_ui() -> Response {
    axum::response::Html(SWAGGER_UI).into_response()
}

async fn healthz() -> Response {
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({ "status": "ok" })),
    )
        .into_response()
}

/// `axum::Json` with the documented `400 {"decode_error": ...}` on failure.
pub struct StrictJson<T>(pub T);

impl<S, T> FromRequest<S> for StrictJson<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(v)) => Ok(StrictJson(v)),
            Err(rejection) => Err(decode_error(rejection.body_text())),
        }
    }
}

pub fn decode_error(message: String) -> Response {
    (
        StatusCode::BAD_REQUEST,
        axum::Json(serde_json::json!({ "decode_error": message })),
    )
        .into_response()
}

fn storage_error(e: &StorageError) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        axum::Json(serde_json::json!({ "storage_error": e.to_string() })),
    )
        .into_response()
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        axum::Json(serde_json::json!({ "error": "NotFound" })),
    )
        .into_response()
}

fn ok_json<T: Serialize>(v: &T) -> Response {
    (StatusCode::OK, axum::Json(v)).into_response()
}

// ---------------------------------------------------------------- evaluate

/// Run `Evaluate` and emit telemetry for it. Returns the outcome untouched.
pub async fn evaluate_with_telemetry(
    state: &AppState,
    request: EvaluateRequest,
) -> Result<PricingResult, EvaluateError> {
    let currency = request.cart.currency.clone();
    let span = evaluate_span(&currency, request.cart.lines.len());
    let started = std::time::Instant::now();
    let use_case = Evaluate {
        catalog: &*state.catalog,
        redemptions: &*state.redemptions,
        clock: &*state.clock,
        events: &*state.events,
    };
    let outcome = use_case
        .run(request.cart, request.customer)
        .instrument(span.clone())
        .await;
    span.in_scope(|| {
        state
            .instruments
            .record_evaluation(&currency, started.elapsed(), &outcome)
    });
    outcome
}

async fn evaluate(
    State(state): State<Arc<AppState>>,
    StrictJson(request): StrictJson<EvaluateRequest>,
) -> Response {
    match evaluate_with_telemetry(&state, request).await {
        Ok(result) => ok_json(&result),
        Err(EvaluateError::Pricing(e)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            axum::Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
        Err(EvaluateError::Storage(e)) => storage_error(&e),
    }
}

// ---------------------------------------------------------------- redemption

fn redemption_error(e: RedemptionError) -> Response {
    match e {
        RedemptionError::NotFound => not_found(),
        RedemptionError::InvalidTransition { from, to } => (
            StatusCode::CONFLICT,
            axum::Json(serde_json::json!({ "error": "InvalidTransition", "from": from, "to": to })),
        )
            .into_response(),
        RedemptionError::Pricing(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            axum::Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
        RedemptionError::Storage(e) => storage_error(&e),
    }
}

async fn redeem(
    State(state): State<Arc<AppState>>,
    StrictJson(request): StrictJson<RedeemRequest>,
) -> Response {
    let currency = request.cart.currency.clone();
    let span = evaluate_span(&currency, request.cart.lines.len());
    let started = std::time::Instant::now();
    let use_case = Redeem {
        catalog: &*state.catalog,
        redemptions: &*state.redemptions,
        clock: &*state.clock,
        events: &*state.events,
    };
    let outcome = use_case
        .run(&request.order_id, request.cart, request.customer)
        .instrument(span.clone())
        .await;
    span.in_scope(|| {
        state
            .instruments
            .record_redemption("reserve", &outcome, started.elapsed(), &currency)
    });
    match outcome {
        Ok(o) => {
            let r = o.redemption();
            ok_json(
                &serde_json::json!({ "order_id": r.order_id, "status": r.status, "result": r.result }),
            )
        }
        Err(e) => redemption_error(e),
    }
}

async fn confirm(State(state): State<Arc<AppState>>, Path(order_id): Path<String>) -> Response {
    let use_case = ConfirmRedemption {
        redemptions: &*state.redemptions,
        clock: &*state.clock,
        events: &*state.events,
    };
    let outcome = use_case.run(&order_id).await;
    state.instruments.record_transition("confirm", &outcome);
    match outcome {
        Ok(r) => ok_json(&serde_json::json!({ "order_id": r.order_id, "status": r.status })),
        Err(e) => redemption_error(e),
    }
}

async fn cancel(State(state): State<Arc<AppState>>, Path(order_id): Path<String>) -> Response {
    let use_case = CancelRedemption {
        redemptions: &*state.redemptions,
        clock: &*state.clock,
        events: &*state.events,
    };
    let outcome = use_case.run(&order_id).await;
    state.instruments.record_transition("cancel", &outcome);
    match outcome {
        Ok(r) => ok_json(&serde_json::json!({ "order_id": r.order_id, "status": r.status })),
        Err(e) => redemption_error(e),
    }
}

async fn get_redemption(
    State(state): State<Arc<AppState>>,
    Path(order_id): Path<String>,
) -> Response {
    match state.redemptions.get(&order_id).await {
        Ok(Some(r)) => ok_json(&r),
        Ok(None) => not_found(),
        Err(e) => redemption_error(e),
    }
}

// ---------------------------------------------------------------- catalogue

async fn get_policy(State(state): State<Arc<AppState>>) -> Response {
    match state.catalog.get_policy().await {
        Ok(p) => ok_json(&p),
        Err(e) => storage_error(&e),
    }
}

async fn put_policy(
    State(state): State<Arc<AppState>>,
    StrictJson(policy): StrictJson<PricingPolicy>,
) -> Response {
    match state.catalog.put_policy(policy.clone()).await {
        Ok(()) => ok_json(&policy),
        Err(e) => storage_error(&e),
    }
}

async fn list_groups(State(state): State<Arc<AppState>>) -> Response {
    match state.catalog.list_groups().await {
        Ok(gs) => ok_json(&gs),
        Err(e) => storage_error(&e),
    }
}

async fn get_group(State(state): State<Arc<AppState>>, Path(group_id): Path<String>) -> Response {
    match state.catalog.get_group(&group_id).await {
        Ok(Some(g)) => ok_json(&g),
        Ok(None) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn put_group(
    State(state): State<Arc<AppState>>,
    Path(group_id): Path<String>,
    StrictJson(mut group): StrictJson<StackingGroup>,
) -> Response {
    group.group_id = group_id;
    match state.catalog.put_group(group.clone()).await {
        Ok(()) => ok_json(&group),
        Err(e) => storage_error(&e),
    }
}

async fn delete_group(
    State(state): State<Arc<AppState>>,
    Path(group_id): Path<String>,
) -> Response {
    match state.catalog.delete_group(&group_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

/// `?source=` on the listings (spec `promotion-catalog`, "Metadata listing and source filter").
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceQuery {
    pub source: Option<String>,
}

async fn list_promotions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SourceQuery>,
) -> Response {
    let ps = match state.catalog.list_promotions().await {
        Ok(ps) => ps,
        Err(e) => return storage_error(&e),
    };
    match q.source {
        None => ok_json(&ps),
        Some(src) => {
            let md = match state.catalog.list_metadata().await {
                Ok(md) => md,
                Err(e) => return storage_error(&e),
            };
            let keep: std::collections::BTreeSet<&str> = md
                .iter()
                .filter(|(_, m)| m.source.as_deref() == Some(src.as_str()))
                .map(|(pid, _)| pid.as_str())
                .collect();
            let filtered: Vec<&Promotion> = ps
                .iter()
                .filter(|p| keep.contains(p.promotion_id.as_str()))
                .collect();
            ok_json(&filtered)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataEntry {
    pub promotion_id: String,
    pub metadata: PromotionMetadata,
}

async fn list_promotion_metadata(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SourceQuery>,
) -> Response {
    match state.catalog.list_metadata().await {
        Ok(md) => {
            let entries: Vec<MetadataEntry> = md
                .into_iter()
                .filter(|(_, m)| q.source.is_none() || m.source == q.source)
                .map(|(promotion_id, metadata)| MetadataEntry {
                    promotion_id,
                    metadata,
                })
                .collect();
            ok_json(&entries)
        }
        Err(e) => storage_error(&e),
    }
}

async fn get_promotion_metadata(
    State(state): State<Arc<AppState>>,
    Path(promotion_id): Path<String>,
) -> Response {
    match state.catalog.get_metadata(&promotion_id).await {
        Ok(Some(m)) => ok_json(&m),
        Ok(None) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn put_promotion_metadata(
    State(state): State<Arc<AppState>>,
    Path(promotion_id): Path<String>,
    StrictJson(metadata): StrictJson<PromotionMetadata>,
) -> Response {
    if let Err(msg) = metadata.check() {
        return decode_error(msg);
    }
    match state
        .catalog
        .put_metadata(&promotion_id, metadata.clone())
        .await
    {
        Ok(true) => ok_json(&metadata),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn delete_promotion_metadata(
    State(state): State<Arc<AppState>>,
    Path(promotion_id): Path<String>,
) -> Response {
    match state.catalog.delete_metadata(&promotion_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

// ---------------------------------------------------------------- allowlists

/// `{customer_ids}` (spec `promotion-catalog`, "Allowlists").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowlistBody {
    pub customer_ids: Vec<String>,
}

pub const MAX_ALLOWLIST: usize = 100_000;

async fn get_allowlist(
    State(state): State<Arc<AppState>>,
    Path(promotion_id): Path<String>,
) -> Response {
    match state.catalog.get_allowlist(&promotion_id).await {
        Ok(Some(customer_ids)) => ok_json(&AllowlistBody { customer_ids }),
        Ok(None) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn put_allowlist(
    State(state): State<Arc<AppState>>,
    Path(promotion_id): Path<String>,
    StrictJson(body): StrictJson<AllowlistBody>,
) -> Response {
    if body.customer_ids.len() > MAX_ALLOWLIST {
        return decode_error(format!(
            "customer_ids must hold at most {MAX_ALLOWLIST} ids"
        ));
    }
    if body.customer_ids.iter().any(|c| c.is_empty()) {
        return decode_error("customer_ids must not contain an empty id".into());
    }
    let mut ids = body.customer_ids;
    ids.sort();
    ids.dedup();
    match state
        .catalog
        .put_allowlist(&promotion_id, ids.clone())
        .await
    {
        Ok(true) => ok_json(&AllowlistBody { customer_ids: ids }),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn delete_allowlist(
    State(state): State<Arc<AppState>>,
    Path(promotion_id): Path<String>,
) -> Response {
    match state.catalog.delete_allowlist(&promotion_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn is_allowlisted(
    State(state): State<Arc<AppState>>,
    Path((promotion_id, customer_id)): Path<(String, String)>,
) -> Response {
    match state
        .catalog
        .is_allowlisted(&promotion_id, &customer_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn add_to_allowlist(
    State(state): State<Arc<AppState>>,
    Path((promotion_id, customer_id)): Path<(String, String)>,
) -> Response {
    match state
        .catalog
        .add_to_allowlist(&promotion_id, &customer_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn remove_from_allowlist(
    State(state): State<Arc<AppState>>,
    Path((promotion_id, customer_id)): Path<(String, String)>,
) -> Response {
    match state
        .catalog
        .remove_from_allowlist(&promotion_id, &customer_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn get_promotion(
    State(state): State<Arc<AppState>>,
    Path(promotion_id): Path<String>,
) -> Response {
    match state.catalog.get_promotion(&promotion_id).await {
        Ok(Some(p)) => ok_json(&p),
        Ok(None) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn put_promotion(
    State(state): State<Arc<AppState>>,
    Path(promotion_id): Path<String>,
    StrictJson(mut promotion): StrictJson<Promotion>,
) -> Response {
    promotion.promotion_id = promotion_id;
    promotion.remaining_budget = None;
    match state.catalog.put_promotion(promotion.clone()).await {
        Ok(()) => ok_json(&promotion),
        Err(e) => storage_error(&e),
    }
}

async fn delete_promotion(
    State(state): State<Arc<AppState>>,
    Path(promotion_id): Path<String>,
) -> Response {
    match state.catalog.archive_promotion(&promotion_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn get_budget(
    State(state): State<Arc<AppState>>,
    Path(promotion_id): Path<String>,
) -> Response {
    match state.catalog.get_budget(&promotion_id).await {
        Ok(Some(remaining)) => ok_json(&BudgetBody { remaining }),
        Ok(None) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn put_budget(
    State(state): State<Arc<AppState>>,
    Path(promotion_id): Path<String>,
    StrictJson(body): StrictJson<BudgetBody>,
) -> Response {
    match state
        .catalog
        .put_budget(&promotion_id, body.remaining.clone())
        .await
    {
        Ok(true) => ok_json(&body),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn list_coupons(State(state): State<Arc<AppState>>) -> Response {
    match state.catalog.list_coupons().await {
        Ok(cs) => ok_json(&cs),
        Err(e) => storage_error(&e),
    }
}

async fn get_coupon(State(state): State<Arc<AppState>>, Path(code): Path<String>) -> Response {
    match state.catalog.get_coupon(&code).await {
        Ok(Some(c)) => ok_json(&c),
        Ok(None) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn put_coupon(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    StrictJson(mut coupon): StrictJson<Coupon>,
) -> Response {
    coupon.code = code;
    coupon.uses = 0; // derived from redemptions, never stored
    match state.catalog.put_coupon(coupon.clone()).await {
        Ok(()) => ok_json(&coupon),
        Err(e) => storage_error(&e),
    }
}

async fn delete_coupon(State(state): State<Arc<AppState>>, Path(code): Path<String>) -> Response {
    match state.catalog.delete_coupon(&code).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn list_rates(State(state): State<Arc<AppState>>) -> Response {
    match state.catalog.list_rates().await {
        Ok(rs) => ok_json(&rs),
        Err(e) => storage_error(&e),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateBody {
    pub numerator: i64,
    pub denominator: i64,
}

async fn put_rate(
    State(state): State<Arc<AppState>>,
    Path((from, to)): Path<(String, String)>,
    StrictJson(body): StrictJson<RateBody>,
) -> Response {
    let rate = ExchangeRate {
        from,
        to,
        numerator: body.numerator,
        denominator: body.denominator,
    };
    match state.catalog.put_rate(rate.clone()).await {
        Ok(()) => ok_json(&rate),
        Err(e) => storage_error(&e),
    }
}

async fn delete_rate(
    State(state): State<Arc<AppState>>,
    Path((from, to)): Path<(String, String)>,
) -> Response {
    match state.catalog.delete_rate(&from, &to).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

// ---------------------------------------------------------------- coupon lifecycle

fn coupon_error(e: CouponError) -> Response {
    match e {
        CouponError::UnknownPromotion | CouponError::NoRuleForTrigger => not_found(),
        CouponError::InvalidCount => decode_error("count must be between 1 and 10000".into()),
        CouponError::CodeSpaceExhausted => (
            StatusCode::CONFLICT,
            axum::Json(serde_json::json!({ "error": "CodeSpaceExhausted" })),
        )
            .into_response(),
        CouponError::Storage(e) => storage_error(&e),
    }
}

async fn generate_coupons(
    State(state): State<Arc<AppState>>,
    StrictJson(req): StrictJson<GenerateCoupons>,
) -> Response {
    let uc = GenerateCouponsUseCase {
        catalog: &*state.catalog,
        codes: &*state.codes,
    };
    match uc.run(req).await {
        Ok(codes) => ok_json(&serde_json::json!({ "codes": codes })),
        Err(e) => coupon_error(e),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantRequest {
    pub trigger: GrantTrigger,
    pub customer_id: String,
    pub order_id: String,
}

async fn grant_coupon(
    State(state): State<Arc<AppState>>,
    StrictJson(req): StrictJson<GrantRequest>,
) -> Response {
    let uc = GrantCoupon {
        catalog: &*state.catalog,
        codes: &*state.codes,
        clock: &*state.clock,
    };
    match uc.run(req.trigger, &req.customer_id, &req.order_id).await {
        Ok(c) => ok_json(&c),
        Err(e) => coupon_error(e),
    }
}

async fn list_grant_rules(State(state): State<Arc<AppState>>) -> Response {
    match state.catalog.list_grant_rules().await {
        Ok(rs) => ok_json(&rs),
        Err(e) => storage_error(&e),
    }
}

async fn get_grant_rule(
    State(state): State<Arc<AppState>>,
    Path(rule_id): Path<String>,
) -> Response {
    match state.catalog.get_grant_rule(&rule_id).await {
        Ok(Some(r)) => ok_json(&r),
        Ok(None) => not_found(),
        Err(e) => storage_error(&e),
    }
}

async fn put_grant_rule(
    State(state): State<Arc<AppState>>,
    Path(rule_id): Path<String>,
    StrictJson(mut rule): StrictJson<GrantRule>,
) -> Response {
    rule.rule_id = rule_id;
    if rule.validity_seconds < 1 || rule.max_uses.is_some_and(|m| m < 0) {
        return decode_error("validity_seconds must be >= 1 and max_uses >= 0".into());
    }
    match state.catalog.put_grant_rule(rule.clone()).await {
        Ok(()) => ok_json(&rule),
        Err(e) => storage_error(&e),
    }
}

async fn delete_grant_rule(
    State(state): State<Arc<AppState>>,
    Path(rule_id): Path<String>,
) -> Response {
    match state.catalog.delete_grant_rule(&rule_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

// ---------------------------------------------------------------- customer coupons

fn customer_coupon_error(e: CustomerCouponError) -> Response {
    match e {
        CustomerCouponError::UnknownCode => not_found(),
        CustomerCouponError::BoundToAnotherCustomer => (
            StatusCode::CONFLICT,
            axum::Json(serde_json::json!({ "error": "BoundToAnotherCustomer" })),
        )
            .into_response(),
        CustomerCouponError::Storage(e) => storage_error(&e),
    }
}

fn list_use_case(state: &AppState) -> ListCustomerCoupons<'_> {
    ListCustomerCoupons {
        catalog: &*state.catalog,
        redemptions: &*state.redemptions,
        clock: &*state.clock,
    }
}

async fn list_customer_coupons(
    State(state): State<Arc<AppState>>,
    Path(customer_id): Path<String>,
    Query(q): Query<SourceQuery>,
) -> Response {
    match list_use_case(&state)
        .run_filtered(&customer_id, q.source.as_deref())
        .await
    {
        Ok(cs) => ok_json(&cs),
        Err(e) => customer_coupon_error(e),
    }
}

async fn save_customer_coupon(
    State(state): State<Arc<AppState>>,
    Path((customer_id, code)): Path<(String, String)>,
) -> Response {
    let uc = SaveCustomerCoupon {
        list: list_use_case(&state),
    };
    match uc.run(&customer_id, &code).await {
        Ok(c) => ok_json(&c),
        Err(e) => customer_coupon_error(e),
    }
}

async fn forget_customer_coupon(
    State(state): State<Arc<AppState>>,
    Path((customer_id, code)): Path<(String, String)>,
) -> Response {
    match state.catalog.forget_coupon(&customer_id, &code).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(e) => storage_error(&e),
    }
}

/// Convenience for tests and tools: the outcome of a reservation as the HTTP body would show it.
pub fn reserve_body(o: &ReserveOutcome) -> serde_json::Value {
    let r = o.redemption();
    serde_json::json!({ "order_id": r.order_id, "status": r.status, "result": r.result })
}
