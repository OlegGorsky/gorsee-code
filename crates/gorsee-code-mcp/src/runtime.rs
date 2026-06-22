use crate::McpError;

pub(crate) fn tokio_runtime() -> Result<tokio::runtime::Runtime, McpError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| McpError::Runtime(error.to_string()))
}
