//! `discount-server`: the HTTP service with OpenTelemetry export.

use std::sync::Arc;

use discount_adapters::http::{AppState, router};
use discount_adapters::telemetry;
use discount_adapters::{Config, InMemoryPromotionRepository, NoopEventPublisher, SystemClock};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let guard = telemetry::init(&config)?;
    let repository = InMemoryPromotionRepository::from_json_file(&config.promotions_path)?;
    tracing::info!(
        listen = %config.listen,
        otlp = %config.otlp_endpoint,
        promotions = repository.promotions.len(),
        groups = repository.groups.len(),
        "discount-server starting"
    );
    let state = Arc::new(AppState {
        repository: Arc::new(repository),
        clock: Arc::new(SystemClock),
        events: Arc::new(NoopEventPublisher),
        policy: config.policy.clone(),
        instruments: guard.instruments(),
    });
    let listener = tokio::net::TcpListener::bind(&config.listen).await?;
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    guard.shutdown();
    Ok(())
}
