//! REST API module for the orchestrator.
//!
//! Provides HTTP API endpoints for managing workloads, nodes, and cluster status.
//!
//! # Endpoints
//!
//! ## Workloads
//! - `POST /api/v1/workloads` - Create a new workload
//! - `GET /api/v1/workloads` - List all workloads
//! - `GET /api/v1/workloads/:id` - Get a specific workload
//! - `PUT /api/v1/workloads/:id` - Update a workload
//! - `DELETE /api/v1/workloads/:id` - Delete a workload
//! - `GET /api/v1/workloads/:id/instances` - List instances for a workload
//!
//! ## Nodes
//! - `GET /api/v1/nodes` - List all nodes
//! - `GET /api/v1/nodes/:id` - Get a specific node
//!
//! ## Cluster
//! - `GET /api/v1/cluster/status` - Get cluster status summary
//!
//! # Authentication
//!
//! All endpoints require Ed25519 request signing. Include these headers:
//! - `X-Auth-PublicKey`: Base64-encoded public key
//! - `X-Auth-Timestamp`: ISO8601 timestamp
//! - `X-Auth-Signature`: Base64-encoded signature
//!
//! See [`auth`] module for signing details.
//!
//! # Example
//!
//! ```no_run
//! use std::sync::Arc;
//! use tokio::sync::mpsc;
//!
//! # #[cfg(feature = "rest-api")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use orchestrator_core::api::{ApiServer, ApiServerConfig, ApiState, AuthConfig};
//! use state_store_interface::in_memory::InMemoryStateStore;
//!
//! // Create state store and other components...
//! let state_store = Arc::new(InMemoryStateStore::new());
//! // ... cluster_manager, workload_tx setup ...
//!
//! // Create API state with auth disabled for development
//! // let state = ApiState::new_without_auth(state_store, cluster_manager, workload_tx);
//!
//! // Or with auth enabled and trusted keys
//! // let auth_config = AuthConfig::default().with_trusted_key(public_key_bytes);
//! // let state = ApiState::new(state_store, cluster_manager, workload_tx, auth_config);
//!
//! // Start server
//! // let server = ApiServer::new(ApiServerConfig::with_port(8080), state);
//! // server.serve().await?;
//! # Ok(())
//! # }
//! ```

pub mod auth;
pub mod error;
pub mod handlers;
pub mod routes;
pub mod state;

pub use auth::{sign_request, AuthConfig, AuthInfo, SignedRequestHeaders};
pub use error::{ApiError, ApiResult};
pub use routes::{build_router, ApiServer, ApiServerConfig};
pub use state::ApiState;
