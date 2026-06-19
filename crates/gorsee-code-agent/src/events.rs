use gorsee_code_core::{Event, EventKind};
use gorsee_code_session::{SessionStore, SessionStoreError};
use serde_json::Value;

pub(crate) struct EventSink<'a> {
    store: &'a SessionStore,
    session_id: String,
    next_sequence: u64,
    count: usize,
}

impl<'a> EventSink<'a> {
    pub(crate) fn new(store: &'a SessionStore, session_id: String) -> Self {
        Self {
            store,
            session_id,
            next_sequence: 1,
            count: 0,
        }
    }

    pub(crate) fn count(&self) -> usize {
        self.count
    }

    pub(crate) fn push(
        &mut self,
        agent_id: Option<&str>,
        kind: EventKind,
        payload: Value,
    ) -> Result<(), SessionStoreError> {
        let event = Event::new(
            self.next_sequence,
            &self.session_id,
            agent_id.map(str::to_string),
            kind,
            payload,
        );
        self.next_sequence += 1;
        self.count += 1;
        self.store.append_event(&event)
    }
}
