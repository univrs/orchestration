//! Health check system for the orchestrator.
//!
//! Provides component health tracking and aggregated health status.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Health status of a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Component is healthy and functioning normally.
    Healthy,
    /// Component is degraded but still operational.
    Degraded,
    /// Component is unhealthy and not functioning.
    Unhealthy,
    /// Component health is unknown.
    Unknown,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

impl HealthStatus {
    /// Check if the status indicates the component is operational.
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }

    /// Get the worst status between two statuses.
    pub fn worst(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unhealthy, _) | (_, Self::Unhealthy) => Self::Unhealthy,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Degraded, _) | (_, Self::Degraded) => Self::Degraded,
            (Self::Healthy, Self::Healthy) => Self::Healthy,
        }
    }
}

/// Health information for a single component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Name of the component.
    pub name: String,
    /// Current health status.
    pub status: HealthStatus,
    /// Optional message describing the health state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// When this health check was last updated.
    pub last_check: DateTime<Utc>,
    /// Additional metadata about the component.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl ComponentHealth {
    /// Create a new healthy component.
    pub fn healthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Healthy,
            message: None,
            last_check: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new degraded component.
    pub fn degraded(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Degraded,
            message: Some(message.into()),
            last_check: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new unhealthy component.
    pub fn unhealthy(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Unhealthy,
            message: Some(message.into()),
            last_check: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the health info.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Aggregated health response for all components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedHealth {
    /// Overall status of the system.
    pub status: HealthStatus,
    /// Service name/identifier.
    pub service: String,
    /// Version of the service.
    pub version: String,
    /// Individual component health statuses.
    pub components: Vec<ComponentHealth>,
    /// Timestamp of this health check.
    pub timestamp: DateTime<Utc>,
}

impl AggregatedHealth {
    /// Check if the overall system is ready to serve traffic.
    pub fn is_ready(&self) -> bool {
        self.status.is_operational()
    }

    /// Check if the system is fully healthy (not just operational).
    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }
}

/// Readiness check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessStatus {
    /// Whether the service is ready to accept traffic.
    pub ready: bool,
    /// Reason for the readiness state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Timestamp of this check.
    pub timestamp: DateTime<Utc>,
}

/// Liveness check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivenessStatus {
    /// Whether the service is alive.
    pub alive: bool,
    /// Timestamp of this check.
    pub timestamp: DateTime<Utc>,
}

/// Health checker that aggregates component health.
#[derive(Clone)]
pub struct HealthChecker {
    inner: Arc<RwLock<HealthCheckerInner>>,
}

struct HealthCheckerInner {
    service_name: String,
    version: String,
    components: HashMap<String, ComponentHealth>,
    ready: bool,
    ready_reason: Option<String>,
}

impl HealthChecker {
    /// Create a new health checker.
    pub fn new(service_name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HealthCheckerInner {
                service_name: service_name.into(),
                version: version.into(),
                components: HashMap::new(),
                ready: false,
                ready_reason: Some("Service starting".to_string()),
            })),
        }
    }

    /// Register or update a component's health.
    pub async fn set_component_health(&self, health: ComponentHealth) {
        let mut inner = self.inner.write().await;
        inner.components.insert(health.name.clone(), health);
    }

    /// Set multiple component health statuses at once.
    pub async fn set_components_health(&self, components: Vec<ComponentHealth>) {
        let mut inner = self.inner.write().await;
        for health in components {
            inner.components.insert(health.name.clone(), health);
        }
    }

    /// Mark the service as ready.
    pub async fn set_ready(&self) {
        let mut inner = self.inner.write().await;
        inner.ready = true;
        inner.ready_reason = None;
    }

    /// Mark the service as not ready with a reason.
    pub async fn set_not_ready(&self, reason: impl Into<String>) {
        let mut inner = self.inner.write().await;
        inner.ready = false;
        inner.ready_reason = Some(reason.into());
    }

    /// Get aggregated health status.
    pub async fn get_health(&self) -> AggregatedHealth {
        let inner = self.inner.read().await;

        let components: Vec<ComponentHealth> = inner.components.values().cloned().collect();

        // Calculate overall status from components
        let status = if components.is_empty() {
            HealthStatus::Unknown
        } else {
            components
                .iter()
                .map(|c| c.status)
                .fold(HealthStatus::Healthy, HealthStatus::worst)
        };

        AggregatedHealth {
            status,
            service: inner.service_name.clone(),
            version: inner.version.clone(),
            components,
            timestamp: Utc::now(),
        }
    }

    /// Get readiness status.
    pub async fn get_readiness(&self) -> ReadinessStatus {
        let inner = self.inner.read().await;
        ReadinessStatus {
            ready: inner.ready,
            reason: inner.ready_reason.clone(),
            timestamp: Utc::now(),
        }
    }

    /// Get liveness status.
    pub async fn get_liveness(&self) -> LivenessStatus {
        // For liveness, we just check if the service is running
        // This is always true if we can respond
        LivenessStatus {
            alive: true,
            timestamp: Utc::now(),
        }
    }

    /// Check if service is ready (simple boolean).
    pub async fn is_ready(&self) -> bool {
        self.inner.read().await.ready
    }

    /// Update a component to healthy status.
    pub async fn mark_healthy(&self, component: impl Into<String>) {
        self.set_component_health(ComponentHealth::healthy(component))
            .await;
    }

    /// Update a component to degraded status.
    pub async fn mark_degraded(&self, component: impl Into<String>, message: impl Into<String>) {
        self.set_component_health(ComponentHealth::degraded(component, message))
            .await;
    }

    /// Update a component to unhealthy status.
    pub async fn mark_unhealthy(&self, component: impl Into<String>, message: impl Into<String>) {
        self.set_component_health(ComponentHealth::unhealthy(component, message))
            .await;
    }

    /// Remove a component from health tracking.
    pub async fn remove_component(&self, component: &str) {
        let mut inner = self.inner.write().await;
        inner.components.remove(component);
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new("orchestrator", env!("CARGO_PKG_VERSION"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_ordering() {
        assert_eq!(
            HealthStatus::Healthy.worst(HealthStatus::Healthy),
            HealthStatus::Healthy
        );
        assert_eq!(
            HealthStatus::Healthy.worst(HealthStatus::Degraded),
            HealthStatus::Degraded
        );
        assert_eq!(
            HealthStatus::Healthy.worst(HealthStatus::Unhealthy),
            HealthStatus::Unhealthy
        );
        assert_eq!(
            HealthStatus::Degraded.worst(HealthStatus::Unhealthy),
            HealthStatus::Unhealthy
        );
    }

    #[test]
    fn test_component_health_creation() {
        let healthy = ComponentHealth::healthy("test");
        assert_eq!(healthy.status, HealthStatus::Healthy);
        assert!(healthy.message.is_none());

        let degraded = ComponentHealth::degraded("test", "slow response");
        assert_eq!(degraded.status, HealthStatus::Degraded);
        assert_eq!(degraded.message.as_deref(), Some("slow response"));

        let unhealthy = ComponentHealth::unhealthy("test", "connection failed");
        assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
        assert_eq!(unhealthy.message.as_deref(), Some("connection failed"));
    }

    #[tokio::test]
    async fn test_health_checker() {
        let checker = HealthChecker::new("test-service", "1.0.0");

        // Initially unknown
        let health = checker.get_health().await;
        assert_eq!(health.status, HealthStatus::Unknown);
        assert_eq!(health.service, "test-service");

        // Add healthy component
        checker.mark_healthy("database").await;
        let health = checker.get_health().await;
        assert_eq!(health.status, HealthStatus::Healthy);

        // Add degraded component
        checker.mark_degraded("cache", "high latency").await;
        let health = checker.get_health().await;
        assert_eq!(health.status, HealthStatus::Degraded);

        // Add unhealthy component
        checker.mark_unhealthy("queue", "connection lost").await;
        let health = checker.get_health().await;
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_readiness() {
        let checker = HealthChecker::new("test", "1.0.0");

        // Initially not ready
        assert!(!checker.is_ready().await);

        // Mark ready
        checker.set_ready().await;
        assert!(checker.is_ready().await);

        // Mark not ready
        checker.set_not_ready("maintenance").await;
        assert!(!checker.is_ready().await);

        let status = checker.get_readiness().await;
        assert!(!status.ready);
        assert_eq!(status.reason.as_deref(), Some("maintenance"));
    }

    #[tokio::test]
    async fn test_liveness() {
        let checker = HealthChecker::new("test", "1.0.0");
        let status = checker.get_liveness().await;
        assert!(status.alive);
    }
}
