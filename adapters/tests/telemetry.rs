//! The `telemetry` spec on in-memory exporters: span attributes, metrics, the trace id on
//! the log record, redemption counters and the budget gauge.

mod common;

use common::*;
use discount_adapters::default_policy;
use discount_adapters::http::{AppState, EvaluateRequest, evaluate_with_telemetry};
use discount_adapters::telemetry::{self, Instruments};
use discount_app::ports::{Redemption, RedemptionError, RedemptionStatus, ReserveOutcome};
use discount_core::*;
use opentelemetry::metrics::MeterProvider;
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::logs::{InMemoryLogExporter, SdkLoggerProvider};
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use std::sync::Arc;
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
