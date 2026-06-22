use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
    ServerHandler, ServiceExt,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EchoRequest {
    text: String,
}

#[derive(Debug, Clone)]
struct FixtureServer {
    tool_router: ToolRouter<Self>,
}

impl FixtureServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl FixtureServer {
    #[tool(description = "Echo text through a real MCP tool")]
    fn echo(&self, Parameters(request): Parameters<EchoRequest>) -> String {
        format!("fixture: {}", request.text)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FixtureServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = FixtureServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
