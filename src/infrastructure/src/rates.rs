//! Exchange-rate source adapters (spec `exchange-rate-source`): the HTTP fetcher over
//! `reqwest`, the refresh entry point shared by the handler and the scheduler, and a canned
//! fetcher for tests and the in-memory server.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use discount_application::ports::RateFetcher;
use discount_application::{
    ExchangeRateSourceStatus, FetchReport, RefreshError, RefreshExchangeRates, is_stale, rates_age,
    source_status,
};
use tracing::Instrument;

use crate::http::AppState;
use crate::telemetry::{self, rate_fetch_span};

/// `GET url` with the source's timeout, three redirects at most, no credentials.
pub struct ReqwestFetcher {
    client: reqwest::Client,
}

impl Default for ReqwestFetcher {
    fn default() -> Self {
        ReqwestFetcher {
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(3))
                .user_agent("discount-server")
                .build()
                .expect("reqwest client"),
        }
    }
}

#[async_trait]
impl RateFetcher for ReqwestFetcher {
    async fn fetch(&self, url: &str, timeout_seconds: i64) -> Result<String, String> {
        let lower = url.to_ascii_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) {
            return Err("url must be http or https".into());
        }
        let res = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(Duration::from_secs(timeout_seconds.max(1) as u64))
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        let status = res.status();
        if !status.is_success() {
            return Err(format!("provider answered HTTP {}", status.as_u16()));
        }
        res.text()
            .await
            .map_err(|e| format!("reading the answer failed: {e}"))
    }
}

/// Answers every fetch with the same result; for tests and the in-memory server.
#[derive(Clone, Default)]
pub struct CannedFetcher(pub Arc<Mutex<Option<Result<String, String>>>>);

impl CannedFetcher {
    pub fn answering(body: Result<String, String>) -> CannedFetcher {
        CannedFetcher(Arc::new(Mutex::new(Some(body))))
    }
    pub fn set(&self, body: Result<String, String>) {
        *self.0.lock().unwrap() = Some(body);
    }
}

#[async_trait]
impl RateFetcher for CannedFetcher {
    async fn fetch(&self, _url: &str, _timeout_seconds: i64) -> Result<String, String> {
        self.0
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| Err("no source configured for this server".into()))
    }
}

/// The status read model as `GET /v1/exchange-rate-source` returns it.
pub async fn status(state: &AppState) -> Result<ExchangeRateSourceStatus, RefreshError> {
    let record = state.catalog.get_rate_source().await?;
    let pairs = if record.is_some() {
        state.catalog.list_source_pairs().await?
    } else {
        vec![]
    };
    Ok(source_status(record, pairs, state.clock.now()))
}

/// Publish the freshness gauges from the stored record (spec `telemetry`).
pub async fn publish_freshness(state: &AppState) {
    let now = state.clock.now();
    match state.catalog.get_rate_source().await {
        Ok(record) => state.instruments.set_rate_freshness(
            record
                .as_ref()
                .map(|r| (rates_age(r, now), is_stale(r, now))),
        ),
        Err(e) => tracing::warn!(error = %e, "exchange-rate source status unavailable"),
    }
}

/// One fetch inside the `discount.exchange_rates.fetch` span, with its metrics and a fresh
/// gauge value afterwards. Used by `POST /v1/exchange-rate-source/refresh` and the scheduler.
pub async fn refresh(state: &AppState) -> Result<FetchReport, RefreshError> {
    let url = match state.catalog.get_rate_source().await {
        Ok(Some(r)) => r.source.url,
        Ok(None) => return Err(RefreshError::NoSource),
        Err(e) => return Err(RefreshError::Storage(e)),
    };
    let span = rate_fetch_span(&url);
    let outcome = async {
        let uc = RefreshExchangeRates {
            catalog: &*state.catalog,
            fetcher: &*state.fetcher,
            clock: &*state.clock,
        };
        let outcome = uc.run().await;
        telemetry::record_rate_fetch_outcome(&outcome);
        state.instruments.record_rate_fetch(&outcome);
        outcome
    }
    .instrument(span)
    .await;
    publish_freshness(state).await;
    outcome
}
