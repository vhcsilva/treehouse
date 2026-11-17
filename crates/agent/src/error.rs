use treehouse_protocol::{codes, RpcError};

/// Erro de uma operação do agente, já com o código JSON-RPC certo.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentError {
    pub code: i64,
    pub message: String,
    pub detail: Option<String>,
}

impl AgentError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), detail: None }
    }
    pub fn git(message: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self { code: codes::GIT_FAILED, message: message.into(), detail: Some(stderr.into()) }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(codes::INVALID_PARAMS, message)
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(codes::INTERNAL, message)
    }
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(d) = &self.detail {
            write!(f, ": {d}")?;
        }
        Ok(())
    }
}

impl std::error::Error for AgentError {}

impl From<AgentError> for RpcError {
    fn from(e: AgentError) -> Self {
        RpcError {
            code: e.code,
            message: e.message,
            data: e.detail.map(serde_json::Value::String),
        }
    }
}

impl From<std::io::Error> for AgentError {
    fn from(e: std::io::Error) -> Self {
        AgentError::internal(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AgentError>;
