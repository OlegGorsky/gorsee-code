mod routes;
mod state;

use std::net::SocketAddr;

use axum::{routing::get, Router};

pub use routes::HealthResponse;
pub use state::GatewayState;

pub fn app(state: GatewayState) -> Router {
    Router::new()
        .route("/health", get(routes::health))
        .route("/v1/sessions", get(routes::sessions))
        .route("/v1/sessions/:id/events", get(routes::session_events))
        .route("/v1/capabilities", get(routes::capabilities))
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
