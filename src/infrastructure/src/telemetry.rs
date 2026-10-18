//! OpenTelemetry instrumentation (spec `telemetry`). Everything here is derived from the
//! `PricingResult`; nothing feeds back into pricing. Exporter failures are logged by the
//! SDK and never reach the request handler.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use discount_application::ports::{Redemption, RedemptionError, ReserveOutcome};
use discount_application::{EvaluateError, FetchReport, RefreshError};
use discount_domain::{ClampReason, Money, PricingError, PricingResult, PromotionId, SkipReason};
use opentelemetry::metrics::{Counter, Histogram, Meter, MeterProvider};
use opentelemetry::trace::TracerProvider;
use opentelemetry::{KeyValue, global};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_opentelemetry::OpenTelemetrySpanExt;
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
pub const METRIC_REDEMPTIONS: &str = "discount.redemptions";
pub const METRIC_BUDGET_REMAINING: &str = "discount.budget.remaining";
pub const METRIC_RATE_FETCHES: &str = "discount.exchange_rates.fetches";
pub const METRIC_RATES_AGE: &str = "discount.exchange_rates.age";
pub const METRIC_RATES_STALE: &str = "discount.exchange_rates.stale";

/// Attribute keys.
pub const ATTR_OUTCOME: &str = "outcome";
pub const ATTR_ERROR_TYPE: &str = "error_type";
pub const ATTR_PROMOTION_ID: &str = "promotion_id";
pub const ATTR_CLAMPED_BY: &str = "clamped_by";
pub const ATTR_REASON: &str = "reason";
pub const ATTR_CURRENCY: &str = "currency";
pub const ATTR_ACTION: &str = "action";

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
pub const SPAN_RATE_FETCH: &str = "discount.exchange_rates.fetch";
pub const SPAN_ATTR_SOURCE_URL: &str = "discount.source.url";
pub const SPAN_ATTR_PAIRS: &str = "discount.pairs";

/// HTTP server span attributes (OpenTelemetry HTTP semantic conventions).
pub const SPAN_ATTR_HTTP_ROUTE: &str = "http.route";
pub const SPAN_ATTR_HTTP_STATUS: &str = "http.response.status_code";

/// Database spans: `db.<repository method>` on the Postgres store (`persistence/postgres.rs`).
pub const SPAN_DB_PREFIX: &str = "db.";
pub const SPAN_ATTR_DB_SYSTEM: &str = "db.system";
pub const SPAN_ATTR_DB_OPERATION: &str = "db.operation.name";

/// Bucket boundaries of `discount.share.bps` (basis points of the subtotal).
pub const SHARE_BPS_BOUNDARIES: [f64; 11] = [
    0.0, 100.0, 250.0, 500.0, 1000.0, 1500.0, 2000.0, 3000.0, 5000.0, 7500.0, 10000.0,
];

/// Every name a dashboard may rely on; `docs/ubiquitous-language.md` lists the same set.
pub const VOCABULARY: [&str; 24] = [
    SPAN_DB_PREFIX,
    SPAN_ATTR_DB_SYSTEM,
    SPAN_ATTR_DB_OPERATION,
    SPAN_ATTR_HTTP_ROUTE,
    SPAN_ATTR_HTTP_STATUS,
    METRIC_REDEMPTIONS,
    METRIC_BUDGET_REMAINING,
    ATTR_ACTION,
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
        PricingError::DuplicateCouponCode { .. } => "DuplicateCouponCode",
        PricingError::InvalidCustomerContext { .. } => "InvalidCustomerContext",
        PricingError::InvalidExchangeRate { .. } => "InvalidExchangeRate",
        PricingError::DuplicateExchangeRate { .. } => "DuplicateExchangeRate",
        PricingError::InvalidRule { .. } => "InvalidRule",
        PricingError::InvalidCoupon { .. } => "InvalidCoupon",
        PricingError::InvalidPromotion { .. } => "InvalidPromotion",
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

/// `(outcome, reason)` of a failed redemption action (spec `telemetry`, "Redemption
/// metrics"): `error` is a storage failure only; everything the caller caused is `rejected`
/// with the variant name as `reason`.
pub type RedemptionOutcome = (&'static str, Option<&'static str>);

pub fn redemption_outcome(e: &RedemptionError) -> RedemptionOutcome {
    match e {
        RedemptionError::Storage(_) => ("error", None),
        RedemptionError::NotFound => ("rejected", Some("NotFound")),
        RedemptionError::InvalidTransition { .. } => ("rejected", Some("InvalidTransition")),
        RedemptionError::Pricing(_) => ("rejected", Some("Pricing")),
    }
}

pub fn skip_reason(r: &SkipReason) -> &'static str {
    match r {
        SkipReason::NotEligible { .. } => "NotEligible",
        SkipReason::ExclusiveApplied { .. } => "ExclusiveApplied",
        SkipReason::FirstMatchTaken { .. } => "FirstMatchTaken",
    }
}

/// Latest known remaining budgets, read by the observable gauge. Refreshed by the service
/// from the catalogue (`Instruments::set_budgets`).
#[derive(Clone, Default)]
pub struct BudgetCache(Arc<Mutex<Vec<(PromotionId, Money)>>>);

/// `(seconds since the last successful fetch, stale)`; the age is absent until a fetch succeeded.
pub type Freshness = (Option<i64>, bool);

/// Freshness of the exchange-rate source for the gauges: `None` while no source is set.
#[derive(Clone, Default)]
pub struct RateFreshness(Arc<Mutex<Option<Freshness>>>);

/// The metric instruments of the service.
#[derive(Clone)]
pub struct Instruments {
    budgets: BudgetCache,
    freshness: RateFreshness,
    rate_fetches: Counter<u64>,
    redemptions: Counter<u64>,
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
        let budgets = BudgetCache::default();
        let cache = budgets.clone();
        let _gauge = meter
            .i64_observable_gauge(METRIC_BUDGET_REMAINING)
            .with_description("Remaining budget per promotion")
            .with_unit("{minor_unit}")
            .with_callback(move |observer| {
                for (pid, m) in cache.0.lock().unwrap().iter() {
                    observer.observe(
                        m.amount,
                        &[
                            KeyValue::new(ATTR_PROMOTION_ID, pid.clone()),
                            KeyValue::new(ATTR_CURRENCY, m.currency.clone()),
                        ],
                    );
                }
            })
            .build();
        let freshness = RateFreshness::default();
        let age_cache = freshness.clone();
        let _age = meter
            .i64_observable_gauge(METRIC_RATES_AGE)
            .with_description("Seconds since the last successful exchange-rate fetch")
            .with_unit("s")
            .with_callback(move |observer| {
                if let Some((Some(age), _)) = *age_cache.0.lock().unwrap() {
                    observer.observe(age, &[]);
                }
            })
            .build();
        let stale_cache = freshness.clone();
        let _stale = meter
            .i64_observable_gauge(METRIC_RATES_STALE)
            .with_description("1 while the exchange rates are older than the source's maximum age")
            .with_unit("1")
            .with_callback(move |observer| {
                if let Some((_, stale)) = *stale_cache.0.lock().unwrap() {
                    observer.observe(i64::from(stale), &[]);
                }
            })
            .build();
        Instruments {
            budgets,
            freshness,
            rate_fetches: meter
                .u64_counter(METRIC_RATE_FETCHES)
                .with_description("Exchange-rate fetches by outcome")
                .with_unit("{fetch}")
                .build(),
            redemptions: meter
                .u64_counter(METRIC_REDEMPTIONS)
                .with_description("Redemption actions by outcome")
                .with_unit("{redemption}")
                .build(),
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

    /// Publish the latest remaining budgets for the gauge.
    pub fn set_budgets(&self, budgets: Vec<(PromotionId, Money)>) {
        *self.budgets.0.lock().unwrap() = budgets;
    }

    /// Publish the exchange-rate freshness for the gauges; `None` removes them.
    pub fn set_rate_freshness(&self, freshness: Option<Freshness>) {
        *self.freshness.0.lock().unwrap() = freshness;
    }

    /// `discount.exchange_rates.fetches{outcome}` for one fetch.
    pub fn record_rate_fetch(&self, outcome: &Result<FetchReport, RefreshError>) {
        let label = if outcome.is_ok() { "ok" } else { "error" };
        self.rate_fetches
            .add(1, &[KeyValue::new(ATTR_OUTCOME, label)]);
    }

    /// `discount.redemptions{action="reserve", outcome}` plus the evaluation telemetry of
    /// the pricing that happened inside the reservation.
    pub fn record_redemption(
        &self,
        action: &'static str,
        outcome: &Result<ReserveOutcome, RedemptionError>,
        elapsed: Duration,
        currency: &str,
    ) {
        let (label, pricing): (
            RedemptionOutcome,
            Option<Result<PricingResult, EvaluateError>>,
        ) = match outcome {
            Ok(ReserveOutcome::Reserved(r)) => (("ok", None), Some(Ok(r.result.clone()))),
            Ok(ReserveOutcome::Idempotent(_)) => (("idempotent", None), None),
            Err(RedemptionError::Pricing(e)) => (
                redemption_outcome(&RedemptionError::Pricing(e.clone())),
                Some(Err(EvaluateError::Pricing(e.clone()))),
            ),
            Err(e) => (redemption_outcome(e), None),
        };
        self.add_redemption(action, label);
        if let Some(p) = pricing {
            self.record_evaluation(currency, elapsed, &p);
        }
    }

    /// `discount.redemptions{action="confirm"|"cancel", outcome}`.
    pub fn record_transition(
        &self,
        action: &'static str,
        outcome: &Result<Redemption, RedemptionError>,
    ) {
        let label = match outcome {
            Ok(_) => ("ok", None),
            Err(e) => redemption_outcome(e),
        };
        self.add_redemption(action, label);
    }

    fn add_redemption(&self, action: &'static str, (outcome, reason): RedemptionOutcome) {
        let mut attrs = vec![
            KeyValue::new(ATTR_ACTION, action),
            KeyValue::new(ATTR_OUTCOME, outcome),
        ];
        if let Some(r) = reason {
            attrs.push(KeyValue::new(ATTR_REASON, r));
        }
        self.redemptions.add(1, &attrs);
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
        // Make the span the current OpenTelemetry context so the log record carries its ids.
        let _cx = span.context().attach();
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
            Err(EvaluateError::Storage(e)) => {
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
/// The `discount.exchange_rates.fetch` span; the URL is recorded without its query string,
/// which may carry a provider key.
pub fn rate_fetch_span(url: &str) -> tracing::Span {
    let shown = url.split('?').next().unwrap_or(url).to_string();
    tracing::info_span!(
        SPAN_RATE_FETCH,
        { SPAN_ATTR_SOURCE_URL } = %shown,
        { SPAN_ATTR_PAIRS } = tracing::field::Empty,
        { SPAN_ATTR_ERROR_TYPE } = tracing::field::Empty,
    )
}

/// Fill the fetch span and log the outcome; must run inside `rate_fetch_span`.
pub fn record_rate_fetch_outcome(outcome: &Result<FetchReport, RefreshError>) {
    let span = tracing::Span::current();
    match outcome {
        Ok(r) => {
            span.record(SPAN_ATTR_PAIRS, r.produced as i64);
            tracing::info!(
                produced = r.produced,
                stored = r.stored,
                "exchange rates fetched"
            );
        }
        Err(e) => {
            let kind = match e {
                RefreshError::NoSource => "NoSource",
                RefreshError::SourceUnavailable(_) => "SourceUnavailable",
                RefreshError::Storage(_) => "Storage",
            };
            span.record(SPAN_ATTR_ERROR_TYPE, kind);
            tracing::warn!(error_type = kind, error = %fetch_error_text(e), "exchange-rate fetch failed");
        }
    }
}

fn fetch_error_text(e: &RefreshError) -> String {
    match e {
        RefreshError::NoSource => "no source".into(),
        RefreshError::SourceUnavailable(m) => m.clone(),
        RefreshError::Storage(s) => s.to_string(),
    }
}

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
/// subscriber (stdout + OpenTelemetry) and the W3C Trace Context propagator. An
/// unreachable endpoint is not an error here: the batch exporters retry and log in the
/// background.
pub fn init(config: &Config) -> Result<TelemetryGuard, Box<dyn std::error::Error>> {
    let res = resource(&config.environment);
    global::set_text_map_propagator(TraceContextPropagator::new());

    let span_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&config.otlp_endpoint)
        .build()?;
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter)
        .with_resource(res.clone())
        .build();
    let tracer = tracer_provider.tracer("discount-infrastructure");
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
    let instruments = Instruments::new(&meter_provider.meter("discount-infrastructure"));

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
            "/../../docs/ubiquitous-language.md"
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

    /// Every repository method of the Postgres store carries the `db.<method>` span
    /// (spec `telemetry`, "Database spans").
    #[test]
    fn every_postgres_method_is_instrumented() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/persistence/postgres.rs"
        ))
        .unwrap();
        let impls = src
            .split("impl PromotionCatalog for PgStore")
            .nth(1)
            .expect("trait impls");
        let mut previous = "";
        for line in impls.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("async fn ") {
                let name = rest.split('(').next().unwrap();
                let expected = format!("#[tracing::instrument(name = \"db.{name}\"");
                assert!(
                    previous.trim_start().starts_with(&expected),
                    "postgres.rs: `{name}` lacks `{expected}`"
                );
            }
            previous = line;
        }
    }
}
