use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    UnknownGame(String),
    InvalidParams(String),
    InvalidScenario(String),
    InvalidLog(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::UnknownGame(g) => write!(f, "unknown game: {g}"),
            EngineError::InvalidParams(m) => write!(f, "invalid params: {m}"),
            EngineError::InvalidScenario(m) => write!(f, "invalid scenario: {m}"),
            EngineError::InvalidLog(m) => write!(f, "invalid action log: {m}"),
        }
    }
}

impl std::error::Error for EngineError {}

pub type EngineResult<T> = Result<T, EngineError>;
