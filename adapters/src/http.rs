//! HTTP adapter: specs `evaluate-http-api`, `redemption` (routes) and `promotion-catalog`
//! (routes).

use std::sync::Arc;

use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Path, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use discount_app::ports::{
    Clock, EventPublisher, PromotionCatalog, RedemptionError, RedemptionRepository, ReserveOutcome,
    StorageError,
};
use discount_app::{CancelRedemption, ConfirmRedemption, Evaluate, EvaluateError, Redeem};
use discount_core::{
    CartSnapshot, Coupon, CustomerContext, ExchangeRate, Money, PricingPolicy, PricingResult,
    Promotion, StackingGroup,
};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing::Instrument;

use crate::telemetry::{Instruments, evaluate_span};

/// Everything a request handler needs. Ports are trait objects so the binary and the tests
/// can plug different implementations.
pub struct AppState {
    pub catalog: Arc<dyn PromotionCatalog>,
    pub redemptions: Arc<dyn RedemptionRepository>,
    pub clock: Arc<dyn Clock>,
    pub events: Arc<dyn EventPublisher>,
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
        .layer(TraceLayer::new_for_http().make_span_with(|req: &Request| {
            tracing::info_span!(
                "http.request",
                otel.name = %format!("{} {}", req.method(), req.uri().path()),
                otel.kind = "server",
                http.request.method = %req.method(),
                url.path = %req.uri().path(),
            )
        }))
        .with_state(state)
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

async fn list_promotions(State(state): State<Arc<AppState>>) -> Response {
    match state.catalog.list_promotions().await {
        Ok(ps) => ok_json(&ps),
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

/// Convenience for tests and tools: the outcome of a reservation as the HTTP body would show it.
pub fn reserve_body(o: &ReserveOutcome) -> serde_json::Value {
    let r = o.redemption();
    serde_json::json!({ "order_id": r.order_id, "status": r.status, "result": r.result })
}
