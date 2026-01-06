//! Kubernetes secret fetching utilities

use k8s_openapi::api::core::v1::Secret;
use kube::{Api, Client};

use crate::{Error, Result};

/// Fetch a secret by name from the given namespace
pub async fn get_secret(client: &Client, namespace: &str, name: &str) -> Result<Secret> {
    let secrets: Api<Secret> = Api::namespaced(client.clone(), namespace);
    secrets
        .get(name)
        .await
        .map_err(|e| Error::KubeError(format!("Failed to get secret {}: {}", name, e)))
}

/// Get a specific key from a secret
pub fn get_secret_key(secret: &Secret, key: &str) -> Result<String> {
    let data = secret
        .data
        .as_ref()
        .ok_or_else(|| Error::SecretError("Secret has no data".to_string()))?;

    let value = data
        .get(key)
        .ok_or_else(|| Error::SecretError(format!("Key '{}' not found in secret", key)))?;

    String::from_utf8(value.0.clone())
        .map_err(|e| Error::SecretError(format!("Invalid UTF-8 in secret key '{}': {}", key, e)))
}
