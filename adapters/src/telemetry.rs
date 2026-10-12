//! OpenTelemetry instrumentation (spec `telemetry`). Everything here is derived from the
//! `PricingResult`; nothing feeds back into pricing. Exporter failures are logged by the
//! SDK and never reach the request handler.

use std::time::Duration;

use discount_app::EvaluateError;
use discount_core::{ClampReason, PricingError, PricingResult, SkipReason};
use opentelemetry::metrics::{Counter, Histogram, Meter, MeterProvider};
use opentelemetry::trace::TracerProvider;
use opentelemetry::{KeyValue, global};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::config::Config;

pub const SERVICE_NAME: &str = "discount";

/// Metric names (ubiquitous language, "Telemetry vocabulary").
pub const METRIC_EVALUATIONS: &str = "discount.evaluations";
pub const METRIC_DURATION: &str = "discount.evaluate.duration";
pub const METRIC_APPLIED: &str = "discount.applied";
pub const METRIC_APPLIED_AMOUNT: &str = "discount.applied.amount";
pub const METRIC_SKIPPED: &str = "discount.skipped";
pub const METRIC_AMOUNT: &str = "discount.amount";
pub const METRIC_SHARE_BPS: &str = "discount.share.bps";

/// Attribute keys.
pub const ATTR_OUTCOME: &str = "outcome";
pub const ATTR_ERROR_TYPE: &str = "error_type";
pub const ATTR_PROMOTION_ID: &str = "promotion_id";
pub const ATTR_CLAMPED_BY: &str = "clamped_by";
pub const ATTR_REASON: &str = "reason";
pub const ATTR_CURRENCY: &str = "currency";

/// Span name and attribute keys.
pub const SPAN_EVALUATE: &str = "discount.evaluate";
pub const SPAN_ATTR_CURRENCY: &str = "discount.currency";
pub const SPAN_ATTR_LINES: &str = "discount.lines";
pub const SPAN_ATTR_PROMOTIONS: &str = "discount.promotions";
pub const SPAN_ATTR_APPLIED: &str = "discount.applied";
pub const SPAN_ATTR_SKIPPED: &str = "discount.skipped";
pub const SPAN_ATTR_ORIGINAL_SUBTOTAL: &str = "discount.original_subtotal";
pub const SPAN_ATTR_TOTAL_DISCOUNT: &str = "discount.total_discount";
pub const SPAN_ATTR_ERROR_TYPE: &str = "discount.error_type";

/// Bucket boundaries of `discount.share.bps` (basis points of the subtotal).
pub const SHARE_BPS_BOUNDARIES: [f64; 11] = [
    0.0, 100.0, 250.0, 500.0, 1000.0, 1500.0, 2000.0, 3000.0, 5000.0, 7500.0, 10000.0,
];

/// Every name a dashboard may rely on; `docs/ubiquitous-language.md` lists the same set.
pub const VOCABULARY: [&str; 16] = [
    METRIC_EVALUATIONS,
    METRIC_DURATION,
    METRIC_APPLIED,
    METRIC_APPLIED_AMOUNT,
    METRIC_SKIPPED,
    METRIC_AMOUNT,
    METRIC_SHARE_BPS,
    ATTR_OUTCOME,
    ATTR_ERROR_TYPE,
    ATTR_PROMOTION_ID,
    ATTR_CLAMPED_BY,
    ATTR_REASON,
    ATTR_CURRENCY,
    SPAN_EVALUATE,
    SPAN_ATTR_ERROR_TYPE,
    SERVICE_NAME,
];

/// The `PricingError` variant name, as used in `error_type` attributes.
pub fn error_type(e: &PricingError) -> &'static str {
    match e {
        PricingError::InvalidCurrency { .. } => "InvalidCurrency",
        PricingError::MixedCurrencies { .. } => "MixedCurrencies",
        PricingError::CurrencyMismatch { .. } => "CurrencyMismatch",
        PricingError::AmountOutOfRange { .. } => "AmountOutOfRange",
        PricingError::InvalidBasisPoints { .. } => "InvalidBasisPoints",
        PricingError::InvalidQuantity { .. } => "InvalidQuantity",
        PricingError::DuplicateLineId { .. } => "DuplicateLineId",
        PricingError::DuplicateGroupId { .. } => "DuplicateGroupId",
        PricingError::DuplicatePromotionId { .. } => "DuplicatePromotionId",
        PricingError::UnknownGroup { .. } => "UnknownGroup",
        PricingError::Overflow => "Overflow",
    }
}

pub fn clamp_reason(c: &Option<ClampReason>) -> &'static str {
    match c {
        None => "none",
        Some(ClampReason::Cap) => "Cap",
        Some(ClampReason::Budget) => "Budget",
        Some(ClampReason::OrderCap) => "OrderCap",
        Some(ClampReason::Running) => "Running",
    }
}

pub fn skip_reason(r: &SkipReason) -> &'static str {
    match r {
        SkipReason::NotEligible { .. } => "NotEligible",
        SkipReason::ExclusiveApplied { .. } => "ExclusiveApplied",
        SkipReason::FirstMatchTaken { .. } => "FirstMatchTaken",
    }
}

/// The metric instruments of the service.
#[derive(Clone)]
pub struct Instruments {
    evaluations: Counter<u64>,
    duration: Histogram<f64>,
    applied: Counter<u64>,
    applied_amount: Counter<u64>,
    skipped: Counter<u64>,
    amount: Histogram<u64>,
    share_bps: Histogram<u64>,
}

impl Instruments {
    pub fn new(meter: &Meter) -> Instruments {
        Instruments {
            evaluations: meter
                .u64_counter(METRIC_EVALUATIONS)
                .with_description("Evaluations by outcome")
                .with_unit("{evaluation}")
                .build(),
            duration: meter
                .f64_histogram(METRIC_DURATION)
                .with_description("Duration of the Evaluate use case")
                .with_unit("s")
                .build(),
            applied: meter
                .u64_counter(METRIC_APPLIED)
                .with_description("Applied promotions by id and clamp reason")
                .with_unit("{promotion}")
                .build(),
            applied_amount: meter
                .u64_counter(METRIC_APPLIED_AMOUNT)
                .with_description("Discount granted per promotion, in minor units")
                .with_unit("{minor_unit}")
                .build(),
            skipped: meter
                .u64_counter(METRIC_SKIPPED)
                .with_description("Skipped promotions by id and reason")
                .with_unit("{promotion}")
                .build(),
            amount: meter
                .u64_histogram(METRIC_AMOUNT)
                .with_description("Total discount per successful evaluation")
                .with_unit("{minor_unit}")
                .build(),
            share_bps: meter
                .u64_histogram(METRIC_SHARE_BPS)
                .with_description("Total discount as basis points of the original subtotal")
                .with_unit("bps")
                .with_boundaries(SHARE_BPS_BOUNDARIES.to_vec())
                .build(),
        }
    }

    /// Instruments on a no-op meter, for tests and tools that do not export.
    pub fn noop() -> Instruments {
        Instruments::new(&opentelemetry::metrics::noop::NoopMeterProvider::new().meter("noop"))
    }

    /// Record everything the `telemetry` spec asks for about one evaluation. Must be called
    /// inside the `discount.evaluate` span so the log record is correlated.
    pub fn record_evaluation(
        &self,
        currency: &str,
        elapsed: Duration,
        outcome: &Result<PricingResult, EvaluateError>,
    ) {
        let span = tracing::Span::current();
        match outcome {
            Ok(r) => {
                let ok = [KeyValue::new(ATTR_OUTCOME, "ok")];
                self.evaluations.add(1, &ok);
                self.duration.record(elapsed.as_secs_f64(), &ok);
                self.amount.record(
                    r.total_discount.amount.max(0) as u64,
                    &[KeyValue::new(ATTR_CURRENCY, currency.to_string())],
                );
                if r.original_subtotal.amount > 0 {
                    let bps = r.total_discount.amount * 10_000 / r.original_subtotal.amount;
                    self.share_bps.record(bps.max(0) as u64, &[]);
                }
                for a in &r.applied {
                    self.applied.add(
                        1,
                        &[
                            KeyValue::new(ATTR_PROMOTION_ID, a.promotion_id.clone()),
                            KeyValue::new(ATTR_CLAMPED_BY, clamp_reason(&a.clamped_by)),
                        ],
                    );
                    self.applied_amount.add(
                        a.discount.amount.max(0) as u64,
                        &[
                            KeyValue::new(ATTR_PROMOTION_ID, a.promotion_id.clone()),
                            KeyValue::new(ATTR_CURRENCY, currency.to_string()),
                        ],
                    );
                }
                for s in &r.skipped {
                    self.skipped.add(
                        1,
                        &[
                            KeyValue::new(ATTR_PROMOTION_ID, s.promotion_id.clone()),
                            KeyValue::new(ATTR_REASON, skip_reason(&s.reason)),
                        ],
                    );
                }
                span.record(
                    SPAN_ATTR_PROMOTIONS,
                    (r.applied.len() + r.skipped.len()) as i64,
                );
                span.record(SPAN_ATTR_APPLIED, r.applied.len() as i64);
                span.record(SPAN_ATTR_SKIPPED, r.skipped.len() as i64);
                span.record(SPAN_ATTR_ORIGINAL_SUBTOTAL, r.original_subtotal.amount);
                span.record(SPAN_ATTR_TOTAL_DISCOUNT, r.total_discount.amount);
                tracing::info!(
                    outcome = "ok",
                    total_discount = r.total_discount.amount,
                    original_subtotal = r.original_subtotal.amount,
                    applied = r.applied.len(),
                    skipped = r.skipped.len(),
                    "evaluation"
                );
            }
            Err(EvaluateError::Pricing(e)) => {
                let et = error_type(e);
                let err = [
                    KeyValue::new(ATTR_OUTCOME, "error"),
                    KeyValue::new(ATTR_ERROR_TYPE, et),
                ];
                self.evaluations.add(1, &err);
                self.duration.record(
                    elapsed.as_secs_f64(),
                    &[KeyValue::new(ATTR_OUTCOME, "error")],
                );
                span.record(SPAN_ATTR_ERROR_TYPE, et);
                tracing::warn!(outcome = "error", error_type = et, "evaluation rejected");
            }
            Err(EvaluateError::Repository(e)) => {
                let err = [
                    KeyValue::new(ATTR_OUTCOME, "error"),
                    KeyValue::new(ATTR_ERROR_TYPE, "Repository"),
                ];
                self.evaluations.add(1, &err);
                self.duration.record(
                    elapsed.as_secs_f64(),
                    &[KeyValue::new(ATTR_OUTCOME, "error")],
                );
                span.record(SPAN_ATTR_ERROR_TYPE, "Repository");
                tracing::error!(outcome = "error", error_type = "Repository", error = ?e, "evaluation failed");
            }
        }
    }
}

/// Open the `discount.evaluate` span with its attributes declared (filled in by
/// `record_evaluation`).
pub fn evaluate_span(currency: &str, lines: usize) -> tracing::Span {
    tracing::info_span!(
        SPAN_EVALUATE,
        "discount.currency" = %currency,
        "discount.lines" = lines as i64,
        "discount.promotions" = tracing::field::Empty,
        "discount.applied" = tracing::field::Empty,
        "discount.skipped" = tracing::field::Empty,
        "discount.original_subtotal" = tracing::field::Empty,
        "discount.total_discount" = tracing::field::Empty,
        "discount.error_type" = tracing::field::Empty,
    )
}

pub fn resource(environment: &str) -> Resource {
    Resource::builder()
        .with_service_name(SERVICE_NAME)
        .with_attributes([
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
            KeyValue::new("deployment.environment", environment.to_string()),
        ])
        .build()
}

/// Providers kept alive for the lifetime of the process; `shutdown` flushes them.
pub struct TelemetryGuard {
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
    logger_provider: SdkLoggerProvider,
    instruments: Instruments,
}

impl TelemetryGuard {
    pub fn instruments(&self) -> Instruments {
        self.instruments.clone()
    }

    pub fn shutdown(self) {
        if let Err(e) = self.tracer_provider.shutdown() {
            eprintln!("tracer shutdown: {e}");
        }
        if let Err(e) = self.meter_provider.shutdown() {
            eprintln!("meter shutdown: {e}");
        }
        if let Err(e) = self.logger_provider.shutdown() {
            eprintln!("logger shutdown: {e}");
        }
    }
}

/// Build the OTLP pipelines for traces, metrics and logs and install the `tracing`
/// subscriber (stdout + OpenTelemetry). An unreachable endpoint is not an error here:
/// the batch exporters retry and log in the background.
pub fn init(config: &Config) -> Result<TelemetryGuard, Box<dyn std::error::Error>> {
    let res = resource(&config.environment);

    let span_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&config.otlp_endpoint)
        .build()?;
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter)
        .with_resource(res.clone())
        .build();
    let tracer = tracer_provider.tracer("discount-adapters");
    global::set_tracer_provider(tracer_provider.clone());

    let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(&config.otlp_endpoint)
        .build()?;
    let meter_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(metric_exporter)
        .with_resource(res.clone())
        .build();
    global::set_meter_provider(meter_provider.clone());
    let instruments = Instruments::new(&meter_provider.meter("discount-adapters"));

    let log_exporter = opentelemetry_otlp::LogExporter::builder()
        .with_tonic()
        .with_endpoint(&config.otlp_endpoint)
        .build()?;
    let logger_provider = SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter)
        .with_resource(res)
        .build();

    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("info,opentelemetry=warn,h2=warn,tonic=warn,hyper=warn")
    });
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .with(OpenTelemetryTracingBridge::new(&logger_provider))
        .try_init()?;

    Ok(TelemetryGuard {
        tracer_provider,
        meter_provider,
        logger_provider,
        instruments,
    })
}

#[cfg(test)]
mod vocabulary_tests {
    use super::VOCABULARY;

    /// The "Telemetry vocabulary" section of the ubiquitous language names every constant.
    #[test]
    fn ubiquitous_language_lists_every_name() {
        let doc = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../docs/ubiquitous-language.md"
        ))
        .unwrap();
        let section = doc
            .split("## Telemetry vocabulary")
            .nth(1)
            .expect("section exists");
        for name in VOCABULARY {
            assert!(
                section.contains(name),
                "ubiquitous-language.md lacks telemetry name `{name}`"
            );
        }
    }
}
