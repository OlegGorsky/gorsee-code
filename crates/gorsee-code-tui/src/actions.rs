use std::{path::Path, sync::Arc};

use anyhow::Result;

type Handler = Arc<dyn Fn(&Path, String) -> Result<String> + Send + Sync>;

#[derive(Clone)]
pub struct TuiHandlers {
    pub(crate) submit: Handler,
    pub(crate) approve: Handler,
    pub(crate) deny: Handler,
    pub(crate) pause: Handler,
    pub(crate) resume: Handler,
    pub(crate) command: Handler,
}

impl TuiHandlers {
    pub fn new<S, A, D, P, R, C>(
        submit: S,
        approve: A,
        deny: D,
        pause: P,
        resume: R,
        command: C,
    ) -> Self
    where
        S: Fn(&Path, String) -> Result<String> + Send + Sync + 'static,
        A: Fn(&Path, String) -> Result<String> + Send + Sync + 'static,
        D: Fn(&Path, String) -> Result<String> + Send + Sync + 'static,
        P: Fn(&Path, String) -> Result<String> + Send + Sync + 'static,
        R: Fn(&Path, String) -> Result<String> + Send + Sync + 'static,
        C: Fn(&Path, String) -> Result<String> + Send + Sync + 'static,
    {
        Self {
            submit: Arc::new(submit),
            approve: Arc::new(approve),
            deny: Arc::new(deny),
            pause: Arc::new(pause),
            resume: Arc::new(resume),
            command: Arc::new(command),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIntent {
    None,
    Submit(String),
    Approve(String),
    Deny(String),
    Pause(String),
    Resume(String),
    Command(String),
    Copy(String),
    Quit,
}
