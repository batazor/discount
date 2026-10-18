//! `discount-server`: the HTTP service with OpenTelemetry export. Postgres when
//! `DATABASE_URL` is set, otherwise an in-memory catalogue seeded from the fixture.

use std::sync::Arc;
use std::time::Duration;

use discount_application::ports::{PromotionCatalog, RedemptionRepository};
use discount_infrastructure::http::{AppState, router};
use discount_infrastructure::telemetry;
use discount_infrastructure::{
    CatalogFixture, Config, InMemoryStore, NoopEventPublisher, PgStore, ReqwestFetcher,
    SystemClock, rates,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let guard = telemetry::init(&config)?;
    let (catalog, redemptions): (Arc<dyn PromotionCatalog>, Arc<dyn RedemptionRepository>) =
        match &config.database_url {
            Some(url) => {
                let store = PgStore::connect(url, &config.policy).await?;
                tracing::info!("catalogue: postgres");
                (Arc::new(store.clone()), Arc::new(store))
            }
            None => {
                let fixture = CatalogFixture::from_json_file(&config.promotions_path)?;
                tracing::info!(
                    promotions = fixture.promotions.len(),
                    groups = fixture.groups.len(),
                    path = %config.promotions_path,
                    "catalogue: in-memory from fixture"
                );
                let store = InMemoryStore::new(config.policy.clone()).with_fixture(fixture);
                (Arc::new(store.clone()), Arc::new(store))
            }
        };
    tracing::info!(listen = %config.listen, otlp = %config.otlp_endpoint, "discount-server starting");
    let instruments = guard.instruments();
    // Budget gauge: refresh the cache from the catalogue every 10 seconds.
    {
        let catalog = catalog.clone();
        let instruments = instruments.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(10));
            loop {
                tick.tick().await;
                match catalog.list_budgets().await {
                    Ok(b) => instruments.set_budgets(b),
                    Err(e) => tracing::warn!(error = %e, "budget refresh failed"),
                }
            }
        });
    }
    let state = Arc::new(AppState {
        catalog,
        redemptions,
        clock: Arc::new(SystemClock),
        events: Arc::new(NoopEventPublisher),
        codes: Arc::new(discount_infrastructure::RandomCodes::default()),
        fetcher: Arc::new(ReqwestFetcher::default()),
        instruments,
    });
    // Exchange-rate source (spec `exchange-rate-source`): every 15 seconds, fetch when the
    // interval has passed since the last attempt, and keep the freshness gauges current.
    {
        let state = state.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(15));
            loop {
                tick.tick().await;
                match state.catalog.get_rate_source().await {
                    Ok(Some(record)) => {
                        if discount_application::fetch_due(&record, state.clock.now()) {
                            let _ = rates::refresh(&state).await;
                        } else {
                            rates::publish_freshness(&state).await;
                        }
                    }
                    Ok(None) => rates::publish_freshness(&state).await,
                    Err(e) => tracing::warn!(error = %e, "exchange-rate source check failed"),
                }
            }
        });
    }
    let listener = tokio::net::TcpListener::bind(&config.listen).await?;
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    guard.shutdown();
    Ok(())
}
