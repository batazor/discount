//! The `telemetry` spec on in-memory exporters: span attributes, metrics, and the trace id
//! on the log record.

use std::sync::Arc;

use discount_adapters::http::{AppState, EvaluateRequest, evaluate_with_telemetry};
use discount_adapters::telemetry::{self, Instruments};
use discount_adapters::{FixedClock, InMemoryPromotionRepository, NoopEventPublisher};
use discount_core::*;
use opentelemetry::metrics::MeterProvider;
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::logs::{InMemoryLogExporter, SdkLoggerProvider};
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
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
    let repo = InMemoryPromotionRepository::from_json_file(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../deploy/promotions.json"
    ))
    .unwrap();
    let state = Arc::new(AppState {
        repository: Arc::new(repo),
        clock: Arc::new(FixedClock(1_760_227_200)),
        events: Arc::new(NoopEventPublisher),
        policy: PricingPolicy {
            rounding: RoundingPolicy::Down,
            stacking: StackingPolicy::Sequential,
            order_cap: OrderCap {
                amount: None,
                bps: None,
            },
        },
        instruments: Instruments::new(&meter_provider.meter("test")),
    });
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

fn eur(a: i64) -> Money {
    Money::new(a, "EUR")
}

fn sample_cart() -> CartSnapshot {
    CartSnapshot {
        currency: "EUR".into(),
        lines: vec![
            CartLine {
                line_id: "L1".into(),
                unit_price: eur(1999),
                quantity: 2,
            },
            CartLine {
                line_id: "L2".into(),
                unit_price: eur(501),
                quantity: 1,
            },
        ],
    }
}

/// Run `f` with a tracing subscriber wired to the harness exporters.
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

fn attr<'a>(kvs: impl Iterator<Item = &'a opentelemetry::KeyValue>, key: &str) -> Option<String> {
    kvs.filter(|kv| kv.key.as_str() == key)
        .map(|kv| kv.value.to_string())
        .next()
}

/// Sum data points of a counter: (attributes as "k=v" sorted, value).
fn counter_points(h: &Harness, name: &str) -> Vec<(Vec<String>, u64)> {
    let mut out = Vec::new();
    for rm in h.metrics.get_finished_metrics().unwrap() {
        for sm in rm.scope_metrics() {
            for m in sm.metrics().filter(|m| m.name() == name) {
                if let AggregatedMetrics::U64(MetricData::Sum(sum)) = m.data() {
                    for dp in sum.data_points() {
                        let mut attrs: Vec<String> = dp
                            .attributes()
                            .map(|kv| format!("{}={}", kv.key.as_str(), kv.value))
                            .collect();
                        attrs.sort();
                        out.push((attrs, dp.value()));
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

#[test]
fn span_metrics_and_log_for_a_priced_cart() {
    let h = harness();
    with_subscriber(&h, || {
        let r = evaluate_with_telemetry(
            &h.state,
            EvaluateRequest {
                cart: sample_cart(),
                customer: CustomerContext {},
            },
        )
        .unwrap();
        assert_eq!(r.total_discount.amount, 749);
    });

    // Span `discount.evaluate` with the spec's attributes.
    let spans = h.spans.get_finished_spans().unwrap();
    let span = spans
        .iter()
        .find(|s| s.name == telemetry::SPAN_EVALUATE)
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

    // Metrics: counters with the documented attributes.
    let ev = counter_points(&h, telemetry::METRIC_EVALUATIONS);
    assert_eq!(ev, vec![(vec!["outcome=ok".to_string()], 1)]);
    let mut applied = counter_points(&h, telemetry::METRIC_APPLIED);
    applied.sort();
    assert_eq!(
        applied,
        vec![
            (
                vec!["clamped_by=Cap".to_string(), "promotion_id=P2".to_string()],
                1
            ),
            (
                vec!["clamped_by=none".to_string(), "promotion_id=P1".to_string()],
                1
            ),
        ]
    );
    assert_eq!(
        counter_points(&h, telemetry::METRIC_SKIPPED),
        vec![(
            vec![
                "promotion_id=P3".to_string(),
                "reason=NotEligible".to_string()
            ],
            1
        )]
    );
    assert_eq!(histogram_count(&h, telemetry::METRIC_DURATION), 1);
    assert_eq!(histogram_count(&h, telemetry::METRIC_AMOUNT), 1);
    assert_eq!(histogram_count(&h, telemetry::METRIC_SHARE_BPS), 1);

    // Log record correlated with the span.
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
    with_subscriber(&h, || {
        let mut cart = sample_cart();
        cart.lines[1].line_id = "L1".into();
        let r = evaluate_with_telemetry(
            &h.state,
            EvaluateRequest {
                cart,
                customer: CustomerContext {},
            },
        );
        assert!(r.is_err());
    });
    let mut ev = counter_points(&h, telemetry::METRIC_EVALUATIONS);
    ev.sort();
    assert_eq!(
        ev,
        vec![(
            vec![
                "error_type=DuplicateLineId".to_string(),
                "outcome=error".to_string()
            ],
            1
        )]
    );
    assert!(counter_points(&h, telemetry::METRIC_APPLIED).is_empty());
    let spans = h.spans.get_finished_spans().unwrap();
    let span = spans
        .iter()
        .find(|s| s.name == telemetry::SPAN_EVALUATE)
        .unwrap();
    assert_eq!(
        attr(span.attributes.iter(), "discount.error_type").as_deref(),
        Some("DuplicateLineId")
    );
}
