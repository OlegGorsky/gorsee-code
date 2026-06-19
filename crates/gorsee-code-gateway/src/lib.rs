mod approval_actions;
mod artifacts;
mod routes;
mod sessions;
mod state;

use std::net::SocketAddr;

use axum::{routing::get, Router};

pub use routes::HealthResponse;
pub use state::GatewayState;

pub fn app(state: GatewayState) -> Router {
    Router::new()
        .route("/health", get(routes::health))
        .route(
            "/v1/sessions",
            get(routes::sessions).post(routes::create_session),
        )
        .route("/v1/sessions/:id", get(routes::session))
        .route("/v1/sessions/:id/events", get(routes::session_events))
        .route(
            "/v1/sessions/:id/message",
            axum::routing::post(routes::post_message),
        )
        .route(
            "/v1/sessions/:id/approve",
            axum::routing::post(routes::approve),
        )
        .route("/v1/sessions/:id/deny", axum::routing::post(routes::deny))
        .route("/v1/sessions/:id/pause", axum::routing::post(routes::pause))
        .route(
            "/v1/sessions/:id/resume",
            axum::routing::post(routes::resume),
        )
        .route("/v1/sessions/:id/usage", get(routes::session_usage))
        .route("/v1/sessions/:id/limits", get(routes::session_limits))
        .route("/v1/sessions/:id/diff", get(routes::session_diff))
        .route("/v1/capabilities", get(routes::capabilities))
        .route("/v1/models", get(routes::capabilities))
        .route("/v1/tools", get(routes::tools))
        .route("/v1/skills", get(routes::skills))
        .route("/v1/hooks", get(routes::hooks))
        .route("/v1/usage", get(routes::usage))
        .route("/v1/limits", get(routes::limits))
        .route("/v1/artifacts", get(routes::artifacts))
        .with_state(state)
}

pub async fn serve(addr: SocketAddr, state: GatewayState) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app(state)).await
}
