use gorsee_code_core::{Event, EventKind};
use gorsee_code_safety::RiskClass;
use gorsee_code_session::{ApprovalRecord, SessionStore, SessionStoreError};
use serde_json::Value;

use crate::AgentRunError;

pub type EventObserver = dyn FnMut(&Event) -> Result<(), AgentRunError> + Send;

pub(crate) struct EventSink<'a> {
    store: &'a SessionStore,
    session_id: String,
    next_sequence: u64,
    count: usize,
    observer: Option<Box<EventObserver>>,
}

impl<'a> EventSink<'a> {
    pub(crate) fn new(store: &'a SessionStore, session_id: String) -> Self {
        Self {
            store,
            session_id,
            next_sequence: 1,
            count: 0,
            observer: None,
        }
    }

    pub(crate) fn resume(
        store: &'a SessionStore,
        session_id: String,
    ) -> Result<Self, SessionStoreError> {
        let events = store.read_events(&session_id)?;
        let next_sequence = events.iter().map(|event| event.sequence).max().unwrap_or(0) + 1;
        Ok(Self {
            store,
            session_id,
            next_sequence,
            count: events.len(),
            observer: None,
        })
    }

    pub(crate) fn with_observer(mut self, observer: Box<EventObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    pub(crate) fn count(&self) -> usize {
        self.count
    }

    pub(crate) fn push(
        &mut self,
        agent_id: Option<&str>,
        kind: EventKind,
        payload: Value,
    ) -> Result<(), AgentRunError> {
        let event = Event::new(
            self.next_sequence,
            &self.session_id,
            agent_id.map(str::to_string),
            kind,
            payload,
        );
        self.store.append_event(&event)?;
        if let Some(observer) = self.observer.as_mut() {
            observer(&event)?;
        }
        self.next_sequence += 1;
        self.count += 1;
        Ok(())
    }

    pub(crate) fn create_approval(
        &self,
        agent_id: &str,
        tool_name: &str,
        args: Value,
        risk: RiskClass,
    ) -> Result<ApprovalRecord, SessionStoreError> {
        let approval = ApprovalRecord::pending(
            &self.session_id,
            self.next_sequence,
            agent_id,
            tool_name,
            args,
            risk,
        );
        self.store.append_approval(&approval)?;
        Ok(approval)
    }
}
