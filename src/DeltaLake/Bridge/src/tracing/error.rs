use std::fmt::{Display, Formatter};
use std::time::Duration;
use crate::tracing::error::TracingError::{InternalError, AlreadyInitialized, Timeout};

#[derive(Debug)]
pub enum TracingError {
    InternalError(String),
    Timeout(Duration),
    AlreadyInitialized,
}

impl Display for TracingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            InternalError(msg) => write!(f, "{}", msg),
            Timeout(duration) => write!(f, "timeout after {:?}", duration),
            AlreadyInitialized => write!(f, "tracing already initialized (ensure runtimes with tracing are disposed before creating more)"),
        }
    }
}

impl std::error::Error for TracingError {}
