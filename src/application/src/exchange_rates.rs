//! Exchange rates from a provider (spec `exchange-rate-source`): fetch a `{base, rates}`
//! table, turn its decimals into the integer fractions the engine takes, store them next to
//! the manual rates, and report freshness. Outside the verified scope; the engine never
//! sees anything but `ExchangeRate`.

use std::collections::BTreeMap;

use discount_domain::{ExchangeRate, MAX_RATE_PART, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::ports::{
    Clock, ExchangeRateSource, ExchangeRateSourceRecord, PromotionCatalog, RateFetcher, SourcePair,
    StorageError,
};

/// Decimal places a provider rate is kept to (design decision 3).
pub const RATE_SCALE_DIGITS: u32 = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshError {
    /// No source is configured.
    NoSource,
    /// The provider could not be fetched or its answer was not a rate table; the message is
    /// what the status shows.
    SourceUnavailable(String),
    Storage(StorageError),
}

impl From<StorageError> for RefreshError {
    fn from(e: StorageError) -> Self {
        RefreshError::Storage(e)
    }
}

/// What one successful fetch did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchReport {
    /// Pairs the provider's table produced (two per entry).
    pub produced: usize,
    /// Pairs written; the rest were `manual` and left alone.
    pub stored: i64,
}

// ---------------------------------------------------------------- provider format

#[derive(Deserialize)]
struct ProviderBody {
    base: String,
    rates: BTreeMap<String, Box<RawValue>>,
}

/// A non-negative decimal as `mantissa / 10^scale`, parsed from JSON number text without
/// floating point: `"1.0873"` is `(10873, 4)`, `"1.2e-5"` is `(12, 6)`.
fn parse_decimal(text: &str) -> Option<(u128, u32)> {
    let t = text.trim();
    let (num, exp) = match t.find(['e', 'E']) {
        Some(i) => (&t[..i], t[i + 1..].parse::<i32>().ok()?),
        None => (t, 0),
    };
    let (int, frac) = match num.find('.') {
        Some(i) => (&num[..i], &num[i + 1..]),
        None => (num, ""),
    };
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    if !int.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let digits = format!("{int}{frac}");
    if digits.len() > 30 {
        return None;
    }
    let mantissa: u128 = if digits.is_empty() {
        0
    } else {
        digits.parse().ok()?
    };
    let scale = frac.len() as i32 - exp;
    if scale >= 0 {
        Some((mantissa, scale as u32))
    } else {
        let shift = (-scale) as u32;
        if shift > 30 {
            return None;
        }
        Some((mantissa.checked_mul(10u128.checked_pow(shift)?)?, 0))
    }
}

/// Round a decimal to `RATE_SCALE_DIGITS` places, half up; the mantissa at that scale.
fn to_scale(mantissa: u128, scale: u32) -> Option<u128> {
    if scale <= RATE_SCALE_DIGITS {
        mantissa.checked_mul(10u128.checked_pow(RATE_SCALE_DIGITS - scale)?)
    } else {
        let div = 10u128.checked_pow(scale - RATE_SCALE_DIGITS)?;
        let (q, r) = (mantissa / div, mantissa % div);
        Some(if r * 2 >= div { q + 1 } else { q })
    }
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

fn in_range(x: u128) -> Option<i64> {
    if x >= 1 && x <= MAX_RATE_PART as u128 {
        Some(x as i64)
    } else {
        None
    }
}

/// `base → X` as `round(r × 10^6) / 10^6` and `X → base` as the flipped fraction, both
/// reduced by their gcd; `None` when a part leaves `1 ..= MAX_RATE_PART`.
fn pairs_for(base: &str, currency: &str, text: &str) -> Option<[ExchangeRate; 2]> {
    let (m, s) = parse_decimal(text)?;
    let num = to_scale(m, s)?;
    let den = 10u128.pow(RATE_SCALE_DIGITS);
    let g = gcd(num, den);
    let (n, d) = (in_range(num / g)?, in_range(den / g)?);
    Some([
        ExchangeRate {
            from: base.to_string(),
            to: currency.to_string(),
            numerator: n,
            denominator: d,
        },
        ExchangeRate {
            from: currency.to_string(),
            to: base.to_string(),
            numerator: d,
            denominator: n,
        },
    ])
}

fn valid_currency(s: &str) -> bool {
    s.len() == 3 && s.bytes().all(|b| b.is_ascii_uppercase())
}

/// Parse a provider body into pairs (spec "Provider format"). The whole table is rejected
/// when any entry is: `Err` carries the reason shown to operators.
pub fn parse_rate_table(body: &str) -> Result<Vec<ExchangeRate>, String> {
    let parsed: ProviderBody = serde_json::from_str(body)
        .map_err(|e| format!("answer is not a {{base, rates}} object: {e}"))?;
    let base = parsed.base.trim().to_ascii_uppercase();
    if !valid_currency(&base) {
        return Err(format!("base '{}' is not a currency", parsed.base));
    }
    if parsed.rates.is_empty() {
        return Err("rates is empty".into());
    }
    let mut out = Vec::with_capacity(parsed.rates.len() * 2);
    for (code, raw) in &parsed.rates {
        let currency = code.trim().to_ascii_uppercase();
        if !valid_currency(&currency) {
            return Err(format!("'{code}' is not a currency"));
        }
        if currency == base {
            continue;
        }
        let text = raw.get();
        let pair = pairs_for(&base, &currency, text).ok_or_else(|| {
            format!("rate {currency} = {text} cannot be stored as a fraction within 1..=10^12")
        })?;
        out.extend(pair);
    }
    if out.is_empty() {
        return Err("rates has no entry other than the base".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------- use case

/// Fetch the configured source once and store what it produced.
pub struct RefreshExchangeRates<'a> {
    pub catalog: &'a dyn PromotionCatalog,
    pub fetcher: &'a dyn RateFetcher,
    pub clock: &'a dyn Clock,
}

impl RefreshExchangeRates<'_> {
    pub async fn run(&self) -> Result<FetchReport, RefreshError> {
        let record = self
            .catalog
            .get_rate_source()
            .await?
            .ok_or(RefreshError::NoSource)?;
        let now = self.clock.now();
        let body = self
            .fetcher
            .fetch(&record.source.url, record.source.timeout_seconds)
            .await;
        let rates = match body.and_then(|b| parse_rate_table(&b)) {
            Ok(r) => r,
            Err(message) => {
                self.catalog
                    .record_rate_fetch(now, Err(message.clone()))
                    .await?;
                return Err(RefreshError::SourceUnavailable(message));
            }
        };
        let produced = rates.len();
        let stored = self.catalog.put_fetched_rates(rates, now).await?;
        self.catalog.record_rate_fetch(now, Ok(stored)).await?;
        Ok(FetchReport { produced, stored })
    }
}

// ---------------------------------------------------------------- status read model

/// `GET /v1/exchange-rate-source`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeRateSourceStatus {
    pub source: Option<ExchangeRateSource>,
    pub updated_at: Option<Timestamp>,
    pub last_attempt_at: Option<Timestamp>,
    pub last_success_at: Option<Timestamp>,
    pub last_error: Option<String>,
    pub last_count: i64,
    pub next_due_at: Option<Timestamp>,
    pub stale: bool,
    pub pairs: Vec<SourcePair>,
}

/// A fetch is due when the interval has passed since the last attempt, or none was made.
pub fn fetch_due(record: &ExchangeRateSourceRecord, now: Timestamp) -> bool {
    match record.last_attempt_at {
        None => true,
        Some(t) => now >= t.saturating_add(record.source.interval_seconds),
    }
}

/// Stale when nothing succeeded within `max_age_seconds` (counted from `updated_at` while
/// nothing has ever succeeded). Pricing never reads this.
pub fn is_stale(record: &ExchangeRateSourceRecord, now: Timestamp) -> bool {
    let since = record.last_success_at.unwrap_or(record.updated_at);
    now.saturating_sub(since) > record.source.max_age_seconds
}

/// Seconds since the last successful fetch, when there was one.
pub fn rates_age(record: &ExchangeRateSourceRecord, now: Timestamp) -> Option<i64> {
    record.last_success_at.map(|t| now.saturating_sub(t).max(0))
}

pub fn source_status(
    record: Option<ExchangeRateSourceRecord>,
    pairs: Vec<SourcePair>,
    now: Timestamp,
) -> ExchangeRateSourceStatus {
    match record {
        None => ExchangeRateSourceStatus {
            source: None,
            updated_at: None,
            last_attempt_at: None,
            last_success_at: None,
            last_error: None,
            last_count: 0,
            next_due_at: None,
            stale: false,
            pairs: vec![],
        },
        Some(r) => ExchangeRateSourceStatus {
            stale: is_stale(&r, now),
            next_due_at: Some(
                r.last_attempt_at
                    .map(|t| t.saturating_add(r.source.interval_seconds))
                    .unwrap_or(now),
            ),
            source: Some(r.source),
            updated_at: Some(r.updated_at),
            last_attempt_at: r.last_attempt_at,
            last_success_at: r.last_success_at,
            last_error: r.last_error,
            last_count: r.last_count,
            pairs,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rate(from: &str, to: &str, n: i64, d: i64) -> ExchangeRate {
        ExchangeRate {
            from: from.into(),
            to: to.into(),
            numerator: n,
            denominator: d,
        }
    }

    #[test]
    fn decimals_are_parsed_without_floats() {
        assert_eq!(parse_decimal("1.0873"), Some((10873, 4)));
        assert_eq!(parse_decimal("162.4"), Some((1624, 1)));
        assert_eq!(parse_decimal("7"), Some((7, 0)));
        assert_eq!(parse_decimal("1.2e-5"), Some((12, 6)));
        assert_eq!(parse_decimal("1.5E2"), Some((150, 0)));
        assert_eq!(parse_decimal("-1"), None);
        assert_eq!(parse_decimal("\"x\""), None);
    }

    #[test]
    fn pairs_from_one_entry_are_reduced_and_flipped() {
        assert_eq!(
            pairs_for("EUR", "USD", "1.0873").unwrap(),
            [
                rate("EUR", "USD", 10873, 10000),
                rate("USD", "EUR", 10000, 10873)
            ]
        );
        assert_eq!(
            pairs_for("EUR", "JPY", "162.4").unwrap(),
            [rate("EUR", "JPY", 812, 5), rate("JPY", "EUR", 5, 812)]
        );
        // Seven decimals round half up to six.
        assert_eq!(
            pairs_for("EUR", "GBP", "0.8654325").unwrap()[0],
            rate("EUR", "GBP", 865433, 1000000)
        );
    }

    #[test]
    fn unrepresentable_rates_are_rejected() {
        assert_eq!(pairs_for("EUR", "X", "0.0000001"), None); // rounds to 0
        assert_eq!(pairs_for("EUR", "X", "0"), None);
        assert_eq!(pairs_for("EUR", "X", "1e13"), None); // numerator above 10^12
        assert_eq!(
            pairs_for("EUR", "X", "10000000").unwrap()[0].numerator,
            10_000_000
        );
    }

    #[test]
    fn provider_body_becomes_a_table() {
        let t = parse_rate_table(
            r#"{"amount":1.0,"base":"EUR","date":"2026-10-18","rates":{"EUR":1,"USD":1.0873,"CHF":0.9412}}"#,
        )
        .unwrap();
        assert_eq!(t.len(), 4, "{t:?}");
        assert!(t.contains(&rate("EUR", "USD", 10873, 10000)));
        assert!(t.contains(&rate("CHF", "EUR", 2500, 2353)));
        assert!(parse_rate_table(r#"{"rates": "n/a"}"#).is_err());
        assert!(parse_rate_table(r#"{"base": "EUR", "rates": {}}"#).is_err());
        assert!(parse_rate_table(r#"{"base": "EUR", "rates": {"USD": "1.08"}}"#).is_err());
        assert!(
            parse_rate_table(r#"{"base": "EUR", "rates": {"usd": 1.08}}"#)
                .unwrap()
                .len()
                == 2
        );
    }

    fn record(interval: i64, max_age: i64) -> ExchangeRateSourceRecord {
        ExchangeRateSourceRecord {
            source: ExchangeRateSource {
                url: "https://rates.example/latest".into(),
                interval_seconds: interval,
                max_age_seconds: max_age,
                timeout_seconds: 10,
            },
            updated_at: 1_000,
            last_attempt_at: None,
            last_success_at: None,
            last_error: None,
            last_count: 0,
        }
    }

    #[test]
    fn due_and_stale_follow_the_setting() {
        let mut r = record(3600, 86_400);
        assert!(fetch_due(&r, 1_000));
        assert!(!is_stale(&r, 1_000 + 86_400));
        assert!(is_stale(&r, 1_000 + 86_401));
        r.last_attempt_at = Some(5_000);
        r.last_success_at = Some(5_000);
        assert!(!fetch_due(&r, 8_599));
        assert!(fetch_due(&r, 8_600));
        assert!(!is_stale(&r, 5_000 + 86_400));
        assert!(is_stale(&r, 5_000 + 86_401));
        assert_eq!(rates_age(&r, 6_000), Some(1_000));
        let s = source_status(Some(r), vec![], 6_000);
        assert_eq!(s.next_due_at, Some(8_600));
        assert!(!s.stale);
        assert_eq!(source_status(None, vec![], 6_000).next_due_at, None);
    }

    #[test]
    fn setting_limits() {
        let ok = record(3600, 86_400).source;
        assert_eq!(ok.check(), Ok(()));
        assert!(
            ExchangeRateSource {
                interval_seconds: 30,
                ..ok.clone()
            }
            .check()
            .is_err()
        );
        assert!(
            ExchangeRateSource {
                max_age_seconds: 3599,
                ..ok.clone()
            }
            .check()
            .is_err()
        );
        assert!(
            ExchangeRateSource {
                timeout_seconds: 0,
                ..ok.clone()
            }
            .check()
            .is_err()
        );
        assert!(
            ExchangeRateSource {
                url: "ftp://x".into(),
                ..ok.clone()
            }
            .check()
            .is_err()
        );
        assert!(
            ExchangeRateSource {
                url: "https://".into(),
                ..ok
            }
            .check()
            .is_err()
        );
    }
}
