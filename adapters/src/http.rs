//! HTTP adapter (spec `evaluate-http-api`): `POST /v1/evaluate`, `GET /healthz`.

use std::sync::Arc;

use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use discount_app::ports::{Clock, EventPublisher, PromotionRepository};
use discount_app::{Evaluate, EvaluateError};
use discount_core::{CartSnapshot, CustomerContext, PricingPolicy, PricingResult};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;

use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::telemetry::{Instruments, evaluate_span};

/// Everything a request handler needs. Ports are trait objects so the binary and the tests
/// can plug different implementations.
pub struct AppState {
    pub repository: Arc<dyn PromotionRepository + Send + Sync>,
    pub clock: Arc<dyn Clock + Send + Sync>,
    pub events: Arc<dyn EventPublisher + Send + Sync>,
    pub policy: PricingPolicy,
    pub instruments: Instruments,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluateRequest {
    pub cart: CartSnapshot,
    pub customer: CustomerContext,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/evaluate", post(evaluate))
        .route("/healthz", get(healthz))
        .layer(TraceLayer::new_for_http().make_span_with(|req: &Request| {
            tracing::info_span!(
                "http.request",
                otel.name = %format!("{} {}", req.method(), req.uri().path()),
                otel.kind = "server",
                http.request.method = %req.method(),
                url.path = %req.uri().path(),
            )
        }))
        .with_state(state)
}

async fn healthz() -> Response {
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({ "status": "ok" })),
    )
        .into_response()
}

/// `axum::Json` with the documented `400 {"decode_error": ...}` on failure.
pub struct StrictJson<T>(pub T);

impl<S, T> FromRequest<S> for StrictJson<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(v)) => Ok(StrictJson(v)),
            Err(rejection) => Err(decode_error(rejection.body_text())),
        }
    }
}

pub fn decode_error(message: String) -> Response {
    (
        StatusCode::BAD_REQUEST,
        axum::Json(serde_json::json!({ "decode_error": message })),
    )
        .into_response()
}

/// Run the use case and emit telemetry for it. Returns the outcome untouched.
pub fn evaluate_with_telemetry(
    state: &AppState,
    request: EvaluateRequest,
) -> Result<PricingResult, EvaluateError> {
    let use_case = Evaluate {
        repository: &*state.repository,
        clock: &*state.clock,
        events: &*state.events,
        policy: state.policy.clone(),
    };
    let currency = request.cart.currency.clone();
    let span = evaluate_span(&currency, request.cart.lines.len());
    let _enter = span.enter();
    // Make the span the current OpenTelemetry context so log records carry its ids.
    let _cx = span.context().attach();
    let started = std::time::Instant::now();
    let outcome = use_case.run(request.cart, request.customer);
    state
        .instruments
        .record_evaluation(&currency, started.elapsed(), &outcome);
    outcome
}

async fn evaluate(
    State(state): State<Arc<AppState>>,
    StrictJson(request): StrictJson<EvaluateRequest>,
) -> Response {
    match evaluate_with_telemetry(&state, request) {
        Ok(result) => (StatusCode::OK, axum::Json(result)).into_response(),
        Err(EvaluateError::Pricing(e)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            axum::Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
        Err(EvaluateError::Repository(e)) => (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({ "repository_error": format!("{e:?}") })),
        )
            .into_response(),
    }
}
