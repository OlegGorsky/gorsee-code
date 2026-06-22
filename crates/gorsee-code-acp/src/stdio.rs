use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use agent_client_protocol::schema::v1::{
    AgentResponse, Error, Implementation, InitializeRequest, InitializeResponse,
    ListSessionsRequest, ListSessionsResponse, NewSessionRequest, NewSessionResponse,
    PromptRequest, PromptResponse, Request, RequestId, StopReason,
};
use gorsee_code_agent::{AgentRunError, EventObserver};
use gorsee_code_core::Event;
use serde_json::{json, Value};

use crate::{
    notifications::{event_notification_lines, response_notification_lines},
    stdio_support::{
        agent_capabilities, create_ready_session, list_sessions, new_acp_session_id, params_as,
        parse_request, prompt_text, runner_error, serialize_error, serialize_params_error,
        serialize_result,
    },
    AcpTurnInput, AcpTurnOutput,
};

pub trait AcpPromptRunner {
    fn run_acp_prompt(&self, input: AcpTurnInput) -> Result<AcpTurnOutput, String>;

    fn run_acp_prompt_observed(
        &self,
        input: AcpTurnInput,
        _observer: Box<EventObserver>,
    ) -> Result<AcpTurnOutput, String> {
        self.run_acp_prompt(input)
    }
}

impl<F> AcpPromptRunner for F
where
    F: Fn(AcpTurnInput) -> Result<AcpTurnOutput, String>,
{
    fn run_acp_prompt(&self, input: AcpTurnInput) -> Result<AcpTurnOutput, String> {
        self(input)
    }
}

#[derive(Debug, Clone)]
pub struct AcpProtocolState {
    default_root: PathBuf,
    sessions: BTreeMap<String, PathBuf>,
}

impl AcpProtocolState {
    pub fn new(default_root: impl AsRef<Path>) -> Self {
        Self {
            default_root: default_root.as_ref().to_path_buf(),
            sessions: BTreeMap::new(),
        }
    }

    pub fn handle_line(
        &mut self,
        line: &str,
        runner: &impl AcpPromptRunner,
    ) -> Result<String, serde_json::Error> {
        let responses = self.handle_line_batch(line, runner)?;
        Ok(responses.into_iter().last().unwrap_or_default())
    }

    pub fn handle_line_batch(
        &mut self,
        line: &str,
        runner: &impl AcpPromptRunner,
    ) -> Result<Vec<String>, serde_json::Error> {
        let request = match parse_request(line) {
            Ok(request) => request,
            Err(error) => return serialize_error(RequestId::Null, error).map(|line| vec![line]),
        };
        self.handle_request_batch(request, runner)
    }

    fn handle_request_batch(
        &mut self,
        request: Request<Value>,
        runner: &impl AcpPromptRunner,
    ) -> Result<Vec<String>, serde_json::Error> {
        if request.method.as_ref() == "session/prompt" {
            return self.handle_prompt(request.id, request.params, runner);
        }
        self.handle_request(request, runner).map(|line| vec![line])
    }

    fn handle_request(
        &mut self,
        request: Request<Value>,
        runner: &impl AcpPromptRunner,
    ) -> Result<String, serde_json::Error> {
        let id = request.id.clone();
        match request.method.as_ref() {
            "initialize" => self.handle_initialize(id, request.params),
            "session/new" => self.handle_new_session(id, request.params),
            "session/list" => self.handle_list_sessions(id, request.params),
            "session/prompt" => self
                .handle_prompt(id, request.params, runner)
                .map(|lines| lines.into_iter().last().unwrap_or_default()),
            _ => serialize_error(id, Error::method_not_found()),
        }
    }

    fn handle_initialize(
        &self,
        id: RequestId,
        params: Option<Value>,
    ) -> Result<String, serde_json::Error> {
        let request = match params_as::<InitializeRequest>(params) {
            Ok(request) => request,
            Err(error) => return serialize_params_error(id, error),
        };
        let response = InitializeResponse::new(request.protocol_version)
            .agent_capabilities(agent_capabilities())
            .agent_info(Implementation::new(
                "gorsee-code",
                env!("CARGO_PKG_VERSION"),
            ));
        serialize_result(id, AgentResponse::InitializeResponse(response))
    }

    fn handle_new_session(
        &mut self,
        id: RequestId,
        params: Option<Value>,
    ) -> Result<String, serde_json::Error> {
        let request = match params_as::<NewSessionRequest>(params) {
            Ok(request) => request,
            Err(error) => return serialize_params_error(id, error),
        };
        if !request.cwd.is_dir() {
            return serialize_error(
                id,
                Error::invalid_params()
                    .data(json!({ "error": format!("workspace does not exist: {}", request.cwd.display()) })),
            );
        }
        let session_id = new_acp_session_id();
        if let Err(error) = create_ready_session(&request.cwd, &session_id) {
            return serialize_error(id, Error::internal_error().data(json!({ "error": error })));
        }
        self.sessions.insert(session_id.clone(), request.cwd);
        serialize_result(
            id,
            AgentResponse::NewSessionResponse(NewSessionResponse::new(session_id)),
        )
    }

    fn handle_list_sessions(
        &self,
        id: RequestId,
        params: Option<Value>,
    ) -> Result<String, serde_json::Error> {
        let request = match params_as::<ListSessionsRequest>(params) {
            Ok(request) => request,
            Err(error) => return serialize_params_error(id, error),
        };
        let root = request.cwd.unwrap_or_else(|| self.default_root.clone());
        let response = ListSessionsResponse::new(list_sessions(&root));
        serialize_result(id, AgentResponse::ListSessionsResponse(response))
    }

    fn handle_prompt(
        &mut self,
        id: RequestId,
        params: Option<Value>,
        runner: &impl AcpPromptRunner,
    ) -> Result<Vec<String>, serde_json::Error> {
        let request = match params_as::<PromptRequest>(params) {
            Ok(request) => request,
            Err(error) => return serialize_params_error(id, error).map(|line| vec![line]),
        };
        let session_id = request.session_id.0.to_string();
        let root = self
            .sessions
            .get(&session_id)
            .cloned()
            .unwrap_or_else(|| self.default_root.clone());
        let input = AcpTurnInput {
            session_id: Some(session_id.clone()),
            workspace_root: root.display().to_string(),
            prompt: prompt_text(&request.prompt),
            user_id: Some("acp-stdio".into()),
        };
        let live_lines = Arc::new(Mutex::new(Vec::new()));
        let live_lines_observer = Arc::clone(&live_lines);
        let observed_session_id = session_id.clone();
        let observer = Box::new(move |event: &Event| {
            let lines = event_notification_lines(&observed_session_id, event)
                .map_err(|error| AgentRunError::Runtime(error.to_string()))?;
            live_lines_observer
                .lock()
                .map_err(|error| AgentRunError::Runtime(error.to_string()))?
                .extend(lines);
            Ok(())
        });
        match runner.run_acp_prompt_observed(input, observer) {
            Ok(output) => {
                let observed = live_lines
                    .lock()
                    .map(|lines| lines.clone())
                    .unwrap_or_default();
                let mut lines = acp_update_lines(&root, &output, observed.is_empty())?;
                lines.extend(observed);
                lines.push(serialize_result(
                    id,
                    AgentResponse::PromptResponse(PromptResponse::new(StopReason::EndTurn)),
                )?);
                Ok(lines)
            }
            Err(error) => serialize_error(id, runner_error(&error)).map(|line| vec![line]),
        }
    }
}

fn acp_update_lines(
    _root: &Path,
    output: &AcpTurnOutput,
    include_transcript: bool,
) -> Result<Vec<String>, serde_json::Error> {
    response_notification_lines(
        &output.summary.session_id,
        &output.response,
        output.orchestration.plan.as_ref(),
        include_transcript,
    )
}
