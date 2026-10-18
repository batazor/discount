//! `discount-server`: the HTTP service with OpenTelemetry export. Postgres when
//! `DATABASE_URL` is set, otherwise an in-memory catalogue seeded from the fixture.

use std::sync::Arc;
use std::time::Duration;

use discount_application::ports::{DEFAULT_TENANT, Tenancy};
use discount_infrastructure::auth::{HttpKeySource, Verifier};
use discount_infrastructure::http::{AppState, AuthMode, router};
use discount_infrastructure::telemetry;
use discount_infrastructure::{
    AuthConfig, CatalogFixture, Config, InMemoryStore, InMemoryTenants, NoopEventPublisher,
    PgTenants, ReqwestFetcher, SystemClock, rates,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let guard = telemetry::init(&config)?;
    let tenancy: Arc<dyn Tenancy> = match &config.database_url {
        Some(url) => {
            let tenants = PgTenants::connect(url, &config.policy).await?;
            tracing::info!("catalogue: postgres");
            Arc::new(tenants)
        }
        None => {
            let fixture = CatalogFixture::from_json_file(&config.promotions_path)?;
            tracing::info!(
                promotions = fixture.promotions.len(),
                groups = fixture.groups.len(),
                path = %config.promotions_path,
                "catalogue: in-memory from fixture (tenant default)"
            );
            let store = InMemoryStore::new(config.policy.clone()).with_fixture(fixture);
            Arc::new(InMemoryTenants::single(store, config.policy.clone()))
        }
    };
    let auth = match &config.auth {
        AuthConfig::Off => {
            tracing::warn!(
                "DISCOUNT_AUTH=off: every request runs as tenant 'default' with every scope; local use only"
            );
            AuthMode::Off
        }
        AuthConfig::Oidc(oidc) => {
            let verifier = Verifier::new(oidc.clone(), Arc::new(HttpKeySource::new()));
            verifier.warm_up().await?;
            tracing::info!(issuers = ?oidc.issuers, "authentication: OIDC bearer tokens");
            AuthMode::Oidc {
                verifier: Arc::new(verifier),
                autocreate_tenants: oidc.autocreate_tenants,
            }
        }
    };
    tracing::info!(listen = %config.listen, otlp = %config.otlp_endpoint, "discount-server starting");
    let instruments = guard.instruments();
    // Budget gauge: refresh the cache from the default tenant's catalogue every 10 seconds.
    // (Per-tenant budget metrics are a telemetry follow-up.)
    {
        let catalog = tenancy.catalog(DEFAULT_TENANT);
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
        tenancy,
        auth,
        clock: Arc::new(SystemClock),
        events: Arc::new(NoopEventPublisher),
        codes: Arc::new(discount_infrastructure::RandomCodes::default()),
        fetcher: Arc::new(ReqwestFetcher::default()),
        instruments,
    });
    // Exchange-rate source (spec `exchange-rate-source`): every 15 seconds, for every
    // tenant, fetch when the interval has passed since the last attempt, and keep the
    // freshness gauges current.
    {
        let state = state.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(15));
            loop {
                tick.tick().await;
                let tenants = match state.tenancy.list_tenants().await {
                    Ok(ts) => ts,
                    Err(e) => {
                        tracing::warn!(error = %e, "exchange-rate source: tenants unavailable");
                        continue;
                    }
                };
                for t in tenants {
                    let catalog = state.tenancy.catalog(&t.tenant_id);
                    match catalog.get_rate_source().await {
                        Ok(Some(record))
                            if discount_application::fetch_due(&record, state.clock.now()) =>
                        {
                            let _ = rates::refresh(
                                &t.tenant_id,
                                &*catalog,
                                &*state.fetcher,
                                &*state.clock,
                                &state.instruments,
                            )
                            .await;
                        }
                        Ok(_) => {
                            rates::publish_freshness(
                                &t.tenant_id,
                                &*catalog,
                                &*state.clock,
                                &state.instruments,
                            )
                            .await
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, tenant_id = %t.tenant_id, "exchange-rate source check failed")
                        }
                    }
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
