//! Error types for the Kafka Partition Remapper Operator

use std::fmt;

/// Result type for the operator
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for the operator
#[derive(Debug)]
pub enum Error {
    /// Kubernetes API error
    KubeError(String),
    /// Configuration error
    ConfigError(String),
    /// Validation error
    ValidationError(String),
    /// Secret error
    SecretError(String),
    /// Finalizer error
    FinalizerError(Box<kube::runtime::finalizer::Error<Error>>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::KubeError(msg) => write!(f, "Kubernetes API error: {}", msg),
            Error::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            Error::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            Error::SecretError(msg) => write!(f, "Secret error: {}", msg),
            Error::FinalizerError(e) => write!(f, "Finalizer error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<kube::runtime::finalizer::Error<Error>> for Error {
    fn from(err: kube::runtime::finalizer::Error<Error>) -> Self {
        Error::FinalizerError(Box::new(err))
    }
}
