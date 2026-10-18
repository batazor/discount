//! The `telemetry` spec on in-memory exporters: span attributes, metrics, the trace id on
//! the log record, redemption counters and the budget gauge.

mod common;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::*;
use discount_application::ports::{Counters, PricingFn, RedemptionRepository, StorageError, Uses};
use discount_application::ports::{PromotionCatalog, Tenancy, Tenant};
use discount_application::ports::{Redemption, RedemptionError, RedemptionStatus, ReserveOutcome};
use discount_domain::*;
use discount_infrastructure::default_policy;
use discount_infrastructure::http::{
    AppState, AuthMode, Ctx, EvaluateRequest, evaluate_with_telemetry, router,
};
use discount_infrastructure::telemetry::{self, Instruments};
use opentelemetry::metrics::MeterProvider;
use opentelemetry::trace::{SpanId, Status, TraceId, TracerProvider};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::logs::{InMemoryLogExporter, SdkLoggerProvider};
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SpanData;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use std::sync::Arc;
use tower::ServiceExt;
use tracing_subscriber::layer::SubscriberExt;

struct Harness {
    spans: InMemorySpanExporter,
    metrics: InMemoryMetricExporter,
    logs: InMemoryLogExporter,
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
    logger_provider: SdkLoggerProvider,
    state: Arc<AppState>,
}

fn harness() -> Harness {
    let spans = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(spans.clone())
        .with_resource(telemetry::resource("test"))
        .build();
    let metrics = InMemoryMetricExporter::default();
    let meter_provider = SdkMeterProvider::builder()
        .with_reader(PeriodicReader::builder(metrics.clone()).build())
        .build();
    let logs = InMemoryLogExporter::default();
    let logger_provider = SdkLoggerProvider::builder()
        .with_simple_exporter(logs.clone())
        .build();
    let state = state_with(
        memory_store(),
        Instruments::new(&meter_provider.meter("test")),
    );
    Harness {
        spans,
        metrics,
        logs,
        tracer_provider,
        meter_provider,
        logger_provider,
        state,
    }
}

/// Run `f` with a tracing subscriber wired to the harness exporters, then flush.
fn with_subscriber(h: &Harness, f: impl FnOnce()) {
    let tracer = h.tracer_provider.tracer("test");
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .with(OpenTelemetryTracingBridge::new(&h.logger_provider));
    tracing::subscriber::with_default(subscriber, f);
    h.tracer_provider.force_flush().unwrap();
    h.logger_provider.force_flush().unwrap();
    h.meter_provider.force_flush().unwrap();
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn attr<'a>(kvs: impl Iterator<Item = &'a opentelemetry::KeyValue>, key: &str) -> Option<String> {
    kvs.filter(|kv| kv.key.as_str() == key)
        .map(|kv| kv.value.to_string())
        .next()
}

fn sorted_attrs<'a>(kvs: impl Iterator<Item = &'a opentelemetry::KeyValue>) -> Vec<String> {
    let mut attrs: Vec<String> = kvs
        .map(|kv| format!("{}={}", kv.key.as_str(), kv.value))
        .collect();
    attrs.sort();
    attrs
}

fn counter_points(h: &Harness, name: &str) -> Vec<(Vec<String>, u64)> {
    let mut out = Vec::new();
    for rm in h.metrics.get_finished_metrics().unwrap() {
        for sm in rm.scope_metrics() {
            for m in sm.metrics().filter(|m| m.name() == name) {
                if let AggregatedMetrics::U64(MetricData::Sum(sum)) = m.data() {
                    for dp in sum.data_points() {
                        out.push((sorted_attrs(dp.attributes()), dp.value()));
                    }
                }
            }
        }
    }
    out.sort();
    out
}

fn gauge_points(h: &Harness, name: &str) -> Vec<(Vec<String>, i64)> {
    let mut out = Vec::new();
    for rm in h.metrics.get_finished_metrics().unwrap() {
        for sm in rm.scope_metrics() {
            for m in sm.metrics().filter(|m| m.name() == name) {
                if let AggregatedMetrics::I64(MetricData::Gauge(g)) = m.data() {
                    for dp in g.data_points() {
                        out.push((sorted_attrs(dp.attributes()), dp.value()));
                    }
                }
            }
        }
    }
    out
}

fn histogram_count(h: &Harness, name: &str) -> u64 {
    let mut total = 0;
    for rm in h.metrics.get_finished_metrics().unwrap() {
        for sm in rm.scope_metrics() {
            for m in sm.metrics().filter(|m| m.name() == name) {
                match m.data() {
                    AggregatedMetrics::F64(MetricData::Histogram(hg)) => {
                        total += hg.data_points().map(|d| d.count()).sum::<u64>()
                    }
                    AggregatedMetrics::U64(MetricData::Histogram(hg)) => {
                        total += hg.data_points().map(|d| d.count()).sum::<u64>()
                    }
                    _ => {}
                }
            }
        }
    }
    total
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn span_metrics_and_log_for_a_priced_cart() {
    let h = harness();
    let rt = runtime();
    with_subscriber(&h, || {
        let r = rt
            .block_on(evaluate_with_telemetry(
                &h.state,
                &Ctx::local(&h.state),
                EvaluateRequest {
                    cart: sample_cart(),
                    customer: CustomerContext::default(),
                },
            ))
            .unwrap();
        assert_eq!(r.total_discount.amount, 749);
    });

    let spans = h.spans.get_finished_spans().unwrap();
    let span = spans
        .iter()
        .find(|sp| sp.name == telemetry::SPAN_EVALUATE)
        .expect("discount.evaluate span");
    let a = |k: &str| attr(span.attributes.iter(), k);
    assert_eq!(a("discount.currency").as_deref(), Some("EUR"));
    assert_eq!(a("discount.lines").as_deref(), Some("2"));
    assert_eq!(a("discount.promotions").as_deref(), Some("3"));
    assert_eq!(a("discount.applied").as_deref(), Some("2"));
    assert_eq!(a("discount.skipped").as_deref(), Some("1"));
    assert_eq!(a("discount.original_subtotal").as_deref(), Some("4499"));
    assert_eq!(a("discount.total_discount").as_deref(), Some("749"));
    assert_eq!(a("discount.error_type"), None);

    assert_eq!(
        counter_points(&h, telemetry::METRIC_EVALUATIONS),
        vec![(s(&["outcome=ok"]), 1)]
    );
    assert_eq!(
        counter_points(&h, telemetry::METRIC_APPLIED),
        vec![
            (s(&["clamped_by=Cap", "promotion_id=P2"]), 1),
            (s(&["clamped_by=none", "promotion_id=P1"]), 1),
        ]
    );
    assert_eq!(
        counter_points(&h, telemetry::METRIC_APPLIED_AMOUNT),
        vec![
            (s(&["currency=EUR", "promotion_id=P1"]), 449),
            (s(&["currency=EUR", "promotion_id=P2"]), 300),
        ]
    );
    assert_eq!(
        counter_points(&h, telemetry::METRIC_SKIPPED),
        vec![(s(&["promotion_id=P3", "reason=NotEligible"]), 1)]
    );
    assert_eq!(histogram_count(&h, telemetry::METRIC_DURATION), 1);
    assert_eq!(histogram_count(&h, telemetry::METRIC_AMOUNT), 1);
    assert_eq!(histogram_count(&h, telemetry::METRIC_SHARE_BPS), 1);

    let logs = h.logs.get_emitted_logs().unwrap();
    let log = logs
        .iter()
        .find(|l| format!("{:?}", l.record.body()).contains("evaluation"))
        .expect("evaluation log");
    let cx = log
        .record
        .trace_context()
        .expect("log carries trace context");
    assert_eq!(cx.trace_id, span.span_context.trace_id());
    assert_eq!(cx.span_id, span.span_context.span_id());
}

#[test]
fn validation_error_is_counted_and_nothing_applied() {
    let h = harness();
    let rt = runtime();
    with_subscriber(&h, || {
        let mut cart = sample_cart();
        cart.lines[1].line_id = "L1".into();
        let r = rt.block_on(evaluate_with_telemetry(
            &h.state,
            &Ctx::local(&h.state),
            EvaluateRequest {
                cart,
                customer: CustomerContext::default(),
            },
        ));
        assert!(r.is_err());
    });
    assert_eq!(
        counter_points(&h, telemetry::METRIC_EVALUATIONS),
        vec![(s(&["error_type=DuplicateLineId", "outcome=error"]), 1)]
    );
    assert!(counter_points(&h, telemetry::METRIC_APPLIED).is_empty());
    let spans = h.spans.get_finished_spans().unwrap();
    let span = spans
        .iter()
        .find(|sp| sp.name == telemetry::SPAN_EVALUATE)
        .unwrap();
    assert_eq!(
        attr(span.attributes.iter(), "discount.error_type").as_deref(),
        Some("DuplicateLineId")
    );
}

#[test]
fn redemption_counter_and_budget_gauge() {
    let h = harness();
    let input = PricingInput {
        now: 0,
        policy: default_policy(),
        customer: CustomerContext::default(),
        cart: sample_cart(),
        coupons: vec![],
        exchange_rates: vec![],
        groups: vec![],
        promotions: vec![],
    };
    let result = apply(&input).unwrap();
    let redemption = Redemption {
        order_id: "o".into(),
        customer_id: None,
        status: RedemptionStatus::Reserved,
        input,
        result,
        created_at: 0,
        updated_at: 0,
    };
    let reserved: Result<ReserveOutcome, RedemptionError> =
        Ok(ReserveOutcome::Reserved(redemption.clone()));
    let idem: Result<ReserveOutcome, RedemptionError> = Ok(ReserveOutcome::Idempotent(redemption));
    with_subscriber(&h, || {
        h.state.instruments.record_redemption(
            "reserve",
            &reserved,
            std::time::Duration::from_millis(1),
            "EUR",
        );
        h.state.instruments.record_redemption(
            "reserve",
            &idem,
            std::time::Duration::from_millis(1),
            "EUR",
        );
        h.state
            .instruments
            .set_budgets(vec![("P1".to_string(), Money::new(700, "EUR"))]);
    });
    assert_eq!(
        counter_points(&h, telemetry::METRIC_REDEMPTIONS),
        vec![
            (s(&["action=reserve", "outcome=idempotent"]), 1),
            (s(&["action=reserve", "outcome=ok"]), 1),
        ]
    );
    assert_eq!(
        counter_points(&h, telemetry::METRIC_EVALUATIONS),
        vec![(s(&["outcome=ok"]), 1)]
    );
    assert_eq!(
        gauge_points(&h, telemetry::METRIC_BUDGET_REMAINING),
        vec![(s(&["currency=EUR", "promotion_id=P1"]), 700)]
    );
}

// ------------------------------------------------------- HTTP server spans

/// Drive the router once under the harness subscriber; returns the response status and
/// the exported spans.
fn serve(h: &Harness, app: axum::Router, req: Request<Body>) -> (StatusCode, Vec<SpanData>) {
    serve_on(&runtime(), h, app, req)
}

/// `serve` on a caller-owned runtime (a Postgres pool must outlive one request).
fn serve_on(
    rt: &tokio::runtime::Runtime,
    h: &Harness,
    app: axum::Router,
    req: Request<Body>,
) -> (StatusCode, Vec<SpanData>) {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    let mut status = StatusCode::IM_A_TEAPOT;
    with_subscriber(h, || {
        status = rt.block_on(app.oneshot(req)).unwrap().status();
    });
    (status, h.spans.get_finished_spans().unwrap())
}

fn request(method: Method, path: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn evaluate_body() -> String {
    serde_json::to_string(&EvaluateRequest {
        cart: sample_cart(),
        customer: CustomerContext::default(),
    })
    .unwrap()
}

fn server_span(spans: &[SpanData]) -> &SpanData {
    spans
        .iter()
        .find(|sp| sp.span_kind == opentelemetry::trace::SpanKind::Server)
        .expect("server span")
}

#[test]
fn server_span_is_named_by_route_template() {
    let h = harness();
    let app = router(h.state.clone());
    let (status, spans) = serve(
        &h,
        app,
        request(Method::POST, "/v1/redemptions/order-42/confirm", ""),
    );
    assert_eq!(status, StatusCode::NOT_FOUND);
    let sp = server_span(&spans);
    assert_eq!(sp.name, "POST /v1/redemptions/{order_id}/confirm");
    let a = |k: &str| attr(sp.attributes.iter(), k);
    assert_eq!(
        a("http.route").as_deref(),
        Some("/v1/redemptions/{order_id}/confirm")
    );
    assert_eq!(
        a("url.path").as_deref(),
        Some("/v1/redemptions/order-42/confirm")
    );
    assert_eq!(a("http.response.status_code").as_deref(), Some("404"));
    assert_eq!(sp.status, Status::Unset);
}

#[test]
fn evaluate_keeps_its_name_and_child_span() {
    let h = harness();
    let app = router(h.state.clone());
    let (status, spans) = serve(
        &h,
        app,
        request(Method::POST, "/v1/evaluate", &evaluate_body()),
    );
    assert_eq!(status, StatusCode::OK);
    let sp = server_span(&spans);
    assert_eq!(sp.name, "POST /v1/evaluate");
    assert_eq!(
        attr(sp.attributes.iter(), "http.response.status_code").as_deref(),
        Some("200")
    );
    let child = spans
        .iter()
        .find(|s| s.name == telemetry::SPAN_EVALUATE)
        .expect("discount.evaluate");
    assert_eq!(child.parent_span_id, sp.span_context.span_id());
    assert_eq!(child.span_context.trace_id(), sp.span_context.trace_id());
}

#[test]
fn unmatched_path_is_named_by_method_only() {
    let h = harness();
    let app = router(h.state.clone());
    let (status, spans) = serve(&h, app, request(Method::GET, "/nope", ""));
    assert_eq!(status, StatusCode::NOT_FOUND);
    let sp = server_span(&spans);
    assert_eq!(sp.name, "GET");
    assert_eq!(attr(sp.attributes.iter(), "http.route"), None);
}

#[test]
fn healthz_is_not_traced() {
    let h = harness();
    let app = router(h.state.clone());
    let (status, spans) = serve(&h, app, request(Method::GET, "/healthz", ""));
    assert_eq!(status, StatusCode::OK);
    assert!(spans.is_empty(), "no span expected, got {spans:?}");
}

#[test]
fn pricing_rejection_keeps_status_unset() {
    let h = harness();
    let app = router(h.state.clone());
    let mut cart = sample_cart();
    cart.currency = "eur".into();
    let body = serde_json::to_string(&EvaluateRequest {
        cart,
        customer: CustomerContext::default(),
    })
    .unwrap();
    let (status, spans) = serve(&h, app, request(Method::POST, "/v1/evaluate", &body));
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let sp = server_span(&spans);
    assert_eq!(
        attr(sp.attributes.iter(), "http.response.status_code").as_deref(),
        Some("422")
    );
    assert_eq!(sp.status, Status::Unset);
    let child = spans
        .iter()
        .find(|s| s.name == telemetry::SPAN_EVALUATE)
        .unwrap();
    assert_eq!(
        attr(child.attributes.iter(), "discount.error_type").as_deref(),
        Some("InvalidCurrency")
    );
}

/// A tenancy whose redemption store is down; the catalogue is the harness's in-memory one.
struct DownTenancy(Arc<dyn PromotionCatalog>);

#[async_trait]
impl Tenancy for DownTenancy {
    fn catalog(&self, _: &str) -> Arc<dyn PromotionCatalog> {
        self.0.clone()
    }
    fn redemptions(&self, _: &str) -> Arc<dyn RedemptionRepository> {
        Arc::new(DownRepository)
    }
    async fn list_tenants(&self) -> Result<Vec<Tenant>, StorageError> {
        Ok(vec![])
    }
    async fn get_tenant(&self, _: &str) -> Result<Option<Tenant>, StorageError> {
        Ok(None)
    }
    async fn put_tenant(&self, _: Tenant) -> Result<(), StorageError> {
        Ok(())
    }
    async fn delete_tenant(&self, _: &str) -> Result<bool, StorageError> {
        Ok(false)
    }
}

/// Every tenant maps to the one test `PgStore` (the tenancy of the Postgres telemetry tests).
struct OneStoreTenancy(Arc<discount_infrastructure::PgStore>);

#[async_trait]
impl Tenancy for OneStoreTenancy {
    fn catalog(&self, _: &str) -> Arc<dyn PromotionCatalog> {
        self.0.clone()
    }
    fn redemptions(&self, _: &str) -> Arc<dyn RedemptionRepository> {
        self.0.clone()
    }
    async fn list_tenants(&self) -> Result<Vec<Tenant>, StorageError> {
        Ok(vec![])
    }
    async fn get_tenant(&self, _: &str) -> Result<Option<Tenant>, StorageError> {
        Ok(None)
    }
    async fn put_tenant(&self, _: Tenant) -> Result<(), StorageError> {
        Ok(())
    }
    async fn delete_tenant(&self, _: &str) -> Result<bool, StorageError> {
        Ok(false)
    }
}

/// A redemption repository whose store is down.
struct DownRepository;

#[async_trait]
impl RedemptionRepository for DownRepository {
    async fn reserve(
        &self,
        _: &str,
        _: Option<&str>,
        _: &[String],
        _: Timestamp,
        _: &PricingFn<'_>,
    ) -> Result<ReserveOutcome, RedemptionError> {
        Err(RedemptionError::Storage(StorageError::Unavailable(
            "down".into(),
        )))
    }
    async fn uses_for_customer(&self, _: &str) -> Result<Uses, RedemptionError> {
        Err(RedemptionError::Storage(StorageError::Unavailable(
            "down".into(),
        )))
    }
    async fn counters(&self, _: &[String]) -> Result<Counters, RedemptionError> {
        Err(RedemptionError::Storage(StorageError::Unavailable(
            "down".into(),
        )))
    }
    async fn coupon_uses_of_customer(
        &self,
        _: &str,
    ) -> Result<Vec<(String, String)>, RedemptionError> {
        Err(RedemptionError::Storage(StorageError::Unavailable(
            "down".into(),
        )))
    }
    async fn transition(
        &self,
        _: &str,
        _: RedemptionStatus,
        _: Timestamp,
    ) -> Result<Redemption, RedemptionError> {
        Err(RedemptionError::Storage(StorageError::Unavailable(
            "down".into(),
        )))
    }
    async fn get(&self, _: &str) -> Result<Option<Redemption>, RedemptionError> {
        Err(RedemptionError::Storage(StorageError::Unavailable(
            "down".into(),
        )))
    }
}

#[test]
fn storage_failure_sets_error_status() {
    let h = harness();
    let state = Arc::new(AppState {
        tenancy: Arc::new(DownTenancy(h.state.tenancy.catalog("default"))),
        auth: AuthMode::Off,
        clock: h.state.clock.clone(),
        events: h.state.events.clone(),
        codes: h.state.codes.clone(),
        instruments: h.state.instruments.clone(),
    });
    let app = router(state);
    let (status, spans) = serve(&h, app, request(Method::GET, "/v1/redemptions/o1", ""));
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let sp = server_span(&spans);
    assert_eq!(sp.name, "GET /v1/redemptions/{order_id}");
    assert_eq!(
        attr(sp.attributes.iter(), "http.response.status_code").as_deref(),
        Some("503")
    );
    assert!(
        matches!(sp.status, Status::Error { .. }),
        "status {:?}",
        sp.status
    );
}

#[test]
fn traceparent_is_honoured() {
    let h = harness();
    let app = router(h.state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/v1/evaluate")
        .header("content-type", "application/json")
        .header(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .body(Body::from(evaluate_body()))
        .unwrap();
    let (status, spans) = serve(&h, app, req);
    assert_eq!(status, StatusCode::OK);
    let sp = server_span(&spans);
    assert_eq!(
        sp.span_context.trace_id(),
        TraceId::from_hex("4bf92f3577b34da6a3ce929d0e0e4736").unwrap()
    );
    assert_eq!(
        sp.parent_span_id,
        SpanId::from_hex("00f067aa0ba902b7").unwrap()
    );
    let child = spans
        .iter()
        .find(|s| s.name == telemetry::SPAN_EVALUATE)
        .unwrap();
    assert_eq!(child.span_context.trace_id(), sp.span_context.trace_id());
}

#[test]
fn malformed_traceparent_starts_a_fresh_trace() {
    let h = harness();
    let app = router(h.state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/v1/evaluate")
        .header("content-type", "application/json")
        .header("traceparent", "garbage")
        .body(Body::from(evaluate_body()))
        .unwrap();
    let (status, spans) = serve(&h, app, req);
    assert_eq!(status, StatusCode::OK);
    let sp = server_span(&spans);
    assert_eq!(sp.parent_span_id, SpanId::INVALID);
    assert_ne!(sp.span_context.trace_id(), TraceId::INVALID);
}

// ------------------------------------------------------------ database spans

/// `PgStore` on the test database (wiped); never skipped (ADR 11).
fn pg_state(rt: &tokio::runtime::Runtime, h: &Harness) -> Arc<AppState> {
    let store = rt.block_on(pg_store());
    let store = Arc::new(store);
    Arc::new(AppState {
        tenancy: Arc::new(OneStoreTenancy(store)),
        auth: AuthMode::Off,
        clock: h.state.clock.clone(),
        events: h.state.events.clone(),
        codes: h.state.codes.clone(),
        instruments: h.state.instruments.clone(),
    })
}

#[test]
fn postgres_calls_produce_db_spans() {
    let h = harness();
    let rt = runtime();
    let state = pg_state(&rt, &h);
    let app = router(state.clone());
    let (status, spans) = serve_on(&rt, &h, app, request(Method::GET, "/v1/policy", ""));
    assert_eq!(status, StatusCode::OK);
    let sp = server_span(&spans);
    let db = spans
        .iter()
        .find(|s| s.name == "db.get_policy")
        .expect("db.get_policy span");
    assert_eq!(db.span_kind, opentelemetry::trace::SpanKind::Client);
    assert_eq!(db.parent_span_id, sp.span_context.span_id());
    assert_eq!(
        attr(db.attributes.iter(), "db.system").as_deref(),
        Some("postgresql")
    );
    assert_eq!(
        attr(db.attributes.iter(), "db.operation.name").as_deref(),
        Some("get_policy")
    );
    assert_eq!(db.status, Status::Unset);

    h.spans.reset();
    let app = router(state);
    let body = serde_json::json!({
        "order_id": "span-1",
        "cart": sample_cart(),
        "customer": CustomerContext::default(),
    })
    .to_string();
    let (status, spans) = serve_on(&rt, &h, app, request(Method::POST, "/v1/redeem", &body));
    assert_eq!(status, StatusCode::OK);
    let sp = server_span(&spans);
    assert_eq!(sp.name, "POST /v1/redeem");
    // The reservation prices inside `discount.evaluate`, so the db span hangs under it.
    let eval = spans
        .iter()
        .find(|s| s.name == telemetry::SPAN_EVALUATE)
        .unwrap();
    assert_eq!(eval.parent_span_id, sp.span_context.span_id());
    let db = spans
        .iter()
        .find(|s| s.name == "db.reserve")
        .expect("db.reserve span");
    assert_eq!(db.parent_span_id, eval.span_context.span_id());
    assert_eq!(db.span_context.trace_id(), sp.span_context.trace_id());
    assert_eq!(
        attr(db.attributes.iter(), "db.operation.name").as_deref(),
        Some("reserve")
    );
}

#[test]
fn transition_rejections_and_storage_errors_are_told_apart() {
    let h = harness();
    let confirmed: Result<Redemption, RedemptionError> = Err(RedemptionError::InvalidTransition {
        from: RedemptionStatus::Confirmed,
        to: RedemptionStatus::Confirmed,
    });
    let missing: Result<Redemption, RedemptionError> = Err(RedemptionError::NotFound);
    let down: Result<ReserveOutcome, RedemptionError> = Err(RedemptionError::Storage(
        StorageError::Unavailable("down".into()),
    ));
    with_subscriber(&h, || {
        h.state.instruments.record_transition("confirm", &confirmed);
        h.state.instruments.record_transition("cancel", &missing);
        h.state.instruments.record_redemption(
            "reserve",
            &down,
            std::time::Duration::from_millis(1),
            "EUR",
        );
    });
    assert_eq!(
        counter_points(&h, telemetry::METRIC_REDEMPTIONS),
        vec![
            (
                s(&["action=cancel", "outcome=rejected", "reason=NotFound"]),
                1
            ),
            (
                s(&[
                    "action=confirm",
                    "outcome=rejected",
                    "reason=InvalidTransition"
                ]),
                1
            ),
            (s(&["action=reserve", "outcome=error"]), 1),
        ]
    );
    assert!(counter_points(&h, telemetry::METRIC_EVALUATIONS).is_empty());
}
