//! Controller implementations for watching and reconciling resources

pub mod remapper_controller;

use kube::Client;
use std::sync::Arc;

/// Shared context for controllers
pub struct Context {
    /// Kubernetes client
    pub client: Client,
}

impl Context {
    /// Create a new context
    pub fn new(client: Client) -> Arc<Self> {
        Arc::new(Self { client })
    }
}
