//! Resource management types for container orchestration scheduling.
//!
//! This module implements the `scheduling.resources` gene from the DOL specification,
//! defining the comprehensive resource model for container orchestration scheduling.
//! It captures the multi-layered resource management hierarchy from node-level capacity
//! down to individual container resource specifications.
//!
//! # Resource Model Overview
//!
//! The resource model enforces several critical invariants:
//! - All resource quantities are non-negative
//! - Limits must be greater than or equal to requests
//! - Allocatable resources cannot exceed capacity
//! - Sum of scheduled pod requests cannot exceed node allocatable
//! - Reserved resources are subtracted from capacity to compute allocatable
//!
//! # Compressible vs Incompressible Resources
//!
//! - **Compressible** (CPU, ephemeral storage): When exceeded, containers are throttled but not terminated
//! - **Incompressible** (memory, disk): When exceeded, containers are OOM-killed
//!
//! # Quality of Service (QoS) Classes
//!
//! - **Guaranteed**: Requests == limits for all resources, highest priority, last to be evicted
//! - **Burstable**: Requests < limits or partial resource specifications, medium priority
//! - **BestEffort**: No requests or limits, first to be evicted but can use all available resources

use std::collections::HashMap;

/// CPU resources measured in millicores (1000m = 1 CPU core).
///
/// This allows fine-grained fractional CPU allocation. For example:
/// - 1000 millicores = 1 full CPU core
/// - 500 millicores = 0.5 CPU cores (half a core)
/// - 2000 millicores = 2 CPU cores
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Millicores(pub u64);

impl Millicores {
    /// Creates a new Millicores from a whole number of cores.
    pub fn from_cores(cores: u64) -> Self {
        Self(cores * 1000)
    }

    /// Converts millicores to fractional cores.
    pub fn to_cores_f64(&self) -> f64 {
        self.0 as f64 / 1000.0
    }
}

/// Memory and storage resources measured in bytes.
///
/// Supports standard binary (Ki, Mi, Gi, Ti) suffixes:
/// - 1 KiB = 1024 bytes
/// - 1 MiB = 1024 KiB
/// - 1 GiB = 1024 MiB
/// - 1 TiB = 1024 GiB
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bytes(pub u64);

impl Bytes {
    pub const KIBI: u64 = 1024;
    pub const MEBI: u64 = 1024 * 1024;
    pub const GIBI: u64 = 1024 * 1024 * 1024;
    pub const TEBI: u64 = 1024 * 1024 * 1024 * 1024;

    /// Creates Bytes from kibibytes.
    pub fn from_kibibytes(kb: u64) -> Self {
        Self(kb * Self::KIBI)
    }

    /// Creates Bytes from mebibytes.
    pub fn from_mebibytes(mb: u64) -> Self {
        Self(mb * Self::MEBI)
    }

    /// Creates Bytes from gibibytes.
    pub fn from_gibibytes(gb: u64) -> Self {
        Self(gb * Self::GIBI)
    }

    /// Creates Bytes from tebibytes.
    pub fn from_tebibytes(tb: u64) -> Self {
        Self(tb * Self::TEBI)
    }
}

/// GPU and other specialized resources measured in integral units.
///
/// Extended resources like GPUs, FPGAs, or InfiniBand adapters must be integral
/// (whole numbers) and are always requested and limited equally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Units(pub u64);

/// Extended resource with a custom resource name and quantity.
///
/// Extended resources enable scheduling of specialized hardware. They are
/// advertised by device plugins and consumed through the same request/limit API.
/// Resource names are unique identifiers (e.g., "nvidia.com/gpu", "example.com/fpga").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedResource {
    /// Unique identifier for this resource type (e.g., "nvidia.com/gpu").
    pub resource_name: String,
    /// Quantity of this resource, must be integral and non-negative.
    pub quantity: u64,
}

/// Node-level resource capacity representing total available hardware resources.
///
/// Capacity represents the total physical or virtualized hardware capabilities
/// including CPU cores, memory, disk storage, GPU units, ephemeral storage,
/// and huge pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeResources {
    /// Total CPU capacity in millicores.
    pub cpu_capacity: Millicores,
    /// Total memory capacity in bytes.
    pub memory_capacity: Bytes,
    /// Total disk storage capacity in bytes.
    pub disk_capacity: Bytes,
    /// Total GPU units available.
    pub gpu_capacity: Units,
    /// Total ephemeral storage capacity in bytes.
    pub ephemeral_storage_capacity: Bytes,
    /// Total huge pages capacity in bytes.
    pub hugepages_capacity: Bytes,
    /// Extended resources available on this node.
    pub extended_resources: Vec<ExtendedResource>,
}

impl NodeResources {
    /// Creates a new NodeResources with all capacities set to zero.
    pub fn zero() -> Self {
        Self {
            cpu_capacity: Millicores(0),
            memory_capacity: Bytes(0),
            disk_capacity: Bytes(0),
            gpu_capacity: Units(0),
            ephemeral_storage_capacity: Bytes(0),
            hugepages_capacity: Bytes(0),
            extended_resources: Vec::new(),
        }
    }
}

/// Node allocatable resources representing what can be allocated to pods.
///
/// The distinction between capacity and allocatable is critical: capacity represents
/// the total hardware resources, while allocatable represents what the scheduler
/// can actually assign to pods. The difference accounts for system daemons, the
/// kubelet, container runtime, and OS processes.
///
/// Invariant: `allocatable = capacity - reserved`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatableResources {
    /// Allocatable CPU in millicores.
    pub cpu_allocatable: Millicores,
    /// Allocatable memory in bytes.
    pub memory_allocatable: Bytes,
    /// Allocatable disk storage in bytes.
    pub disk_allocatable: Bytes,
    /// Allocatable GPU units.
    pub gpu_allocatable: Units,
    /// Allocatable ephemeral storage in bytes.
    pub ephemeral_storage_allocatable: Bytes,
    /// Allocatable huge pages in bytes.
    pub hugepages_allocatable: Bytes,
    /// Extended resources available for allocation.
    pub extended_resources: Vec<ExtendedResource>,
}

impl AllocatableResources {
    /// Creates a new AllocatableResources with all values set to zero.
    pub fn zero() -> Self {
        Self {
            cpu_allocatable: Millicores(0),
            memory_allocatable: Bytes(0),
            disk_allocatable: Bytes(0),
            gpu_allocatable: Units(0),
            ephemeral_storage_allocatable: Bytes(0),
            hugepages_allocatable: Bytes(0),
            extended_resources: Vec::new(),
        }
    }
}

/// Node reserved resources for system daemons and OS processes.
///
/// Reserved resources account for the kubelet, container runtime, system daemons,
/// and OS processes that must run to maintain node functionality. These resources
/// are subtracted from capacity to compute allocatable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservedResources {
    /// Reserved CPU in millicores.
    pub cpu_reserved: Millicores,
    /// Reserved memory in bytes.
    pub memory_reserved: Bytes,
    /// Reserved disk storage in bytes.
    pub disk_reserved: Bytes,
}

impl ReservedResources {
    /// Creates a new ReservedResources with all values set to zero.
    pub fn zero() -> Self {
        Self {
            cpu_reserved: Millicores(0),
            memory_reserved: Bytes(0),
            disk_reserved: Bytes(0),
        }
    }
}

/// Container resource requests and limits.
///
/// Resource requests represent the minimum guaranteed resources a container needs.
/// The scheduler uses requests to make placement decisions. Requests are guarantees
/// that the container will receive at least this amount of resources.
///
/// Resource limits represent the maximum resources a container can consume. Limits
/// prevent resource starvation and enable resource overcommitment.
///
/// Invariant: `limit >= request` for all resources
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerResources {
    /// Minimum guaranteed CPU (request).
    pub cpu_request: Option<Millicores>,
    /// Minimum guaranteed memory (request).
    pub memory_request: Option<Bytes>,
    /// Minimum guaranteed ephemeral storage (request).
    pub ephemeral_storage_request: Option<Bytes>,
    /// Minimum guaranteed huge pages (request).
    pub hugepages_request: Option<Bytes>,

    /// Maximum allowed CPU (limit).
    pub cpu_limit: Option<Millicores>,
    /// Maximum allowed memory (limit).
    pub memory_limit: Option<Bytes>,
    /// Maximum allowed ephemeral storage (limit).
    pub ephemeral_storage_limit: Option<Bytes>,
    /// Maximum allowed huge pages (limit).
    pub hugepages_limit: Option<Bytes>,

    /// Extended resource requests.
    pub extended_resource_requests: HashMap<String, u64>,
    /// Extended resource limits.
    pub extended_resource_limits: HashMap<String, u64>,
}

impl ContainerResources {
    /// Creates a new ContainerResources with no requests or limits.
    pub fn empty() -> Self {
        Self {
            cpu_request: None,
            memory_request: None,
            ephemeral_storage_request: None,
            hugepages_request: None,
            cpu_limit: None,
            memory_limit: None,
            ephemeral_storage_limit: None,
            hugepages_limit: None,
            extended_resource_requests: HashMap::new(),
            extended_resource_limits: HashMap::new(),
        }
    }

    /// Validates that all limits are greater than or equal to their corresponding requests.
    ///
    /// Returns an error message if validation fails, None if valid.
    pub fn validate(&self) -> Option<String> {
        // Check CPU
        if let (Some(request), Some(limit)) = (self.cpu_request, self.cpu_limit) {
            if limit < request {
                return Some(format!(
                    "CPU limit ({}) must be >= request ({})",
                    limit.0, request.0
                ));
            }
        }

        // Check memory
        if let (Some(request), Some(limit)) = (self.memory_request, self.memory_limit) {
            if limit < request {
                return Some(format!(
                    "Memory limit ({}) must be >= request ({})",
                    limit.0, request.0
                ));
            }
        }

        // Check ephemeral storage
        if let (Some(request), Some(limit)) =
            (self.ephemeral_storage_request, self.ephemeral_storage_limit)
        {
            if limit < request {
                return Some(format!(
                    "Ephemeral storage limit ({}) must be >= request ({})",
                    limit.0, request.0
                ));
            }
        }

        // Check huge pages
        if let (Some(request), Some(limit)) = (self.hugepages_request, self.hugepages_limit) {
            if limit < request {
                return Some(format!(
                    "Huge pages limit ({}) must be >= request ({})",
                    limit.0, request.0
                ));
            }
        }

        None
    }
}

/// Pod-level aggregated resources computed from all containers in the pod.
///
/// Pod total requests and limits are computed as the sum of all container requests
/// and limits within the pod. The scheduler evaluates pod-level totals against node
/// allocatable resources to determine scheduling feasibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodResources {
    /// Sum of all container CPU requests.
    pub total_cpu_request: Millicores,
    /// Sum of all container memory requests.
    pub total_memory_request: Bytes,
    /// Sum of all container CPU limits.
    pub total_cpu_limit: Option<Millicores>,
    /// Sum of all container memory limits.
    pub total_memory_limit: Option<Bytes>,
    /// Aggregated extended resource requests.
    pub extended_resource_requests: HashMap<String, u64>,
    /// Aggregated extended resource limits.
    pub extended_resource_limits: HashMap<String, u64>,
}

impl PodResources {
    /// Derives pod-level resources from a collection of container resources.
    ///
    /// This method aggregates all container requests and limits into pod-level totals.
    /// For standard resources, requests are summed; limits are summed only if all
    /// containers specify limits (otherwise None).
    pub fn from_containers(containers: &[ContainerResources]) -> Self {
        let mut total_cpu_request = 0u64;
        let mut total_memory_request = 0u64;
        let mut total_cpu_limit = Some(0u64);
        let mut total_memory_limit = Some(0u64);
        let mut extended_requests: HashMap<String, u64> = HashMap::new();
        let mut extended_limits: HashMap<String, u64> = HashMap::new();

        for container in containers {
            // Sum CPU requests
            if let Some(cpu_req) = container.cpu_request {
                total_cpu_request += cpu_req.0;
            }

            // Sum memory requests
            if let Some(mem_req) = container.memory_request {
                total_memory_request += mem_req.0;
            }

            // Sum CPU limits (if all have limits)
            match (total_cpu_limit, container.cpu_limit) {
                (Some(sum), Some(limit)) => total_cpu_limit = Some(sum + limit.0),
                _ => total_cpu_limit = None,
            }

            // Sum memory limits (if all have limits)
            match (total_memory_limit, container.memory_limit) {
                (Some(sum), Some(limit)) => total_memory_limit = Some(sum + limit.0),
                _ => total_memory_limit = None,
            }

            // Aggregate extended resource requests
            for (name, quantity) in &container.extended_resource_requests {
                *extended_requests.entry(name.clone()).or_insert(0) += quantity;
            }

            // Aggregate extended resource limits
            for (name, quantity) in &container.extended_resource_limits {
                *extended_limits.entry(name.clone()).or_insert(0) += quantity;
            }
        }

        Self {
            total_cpu_request: Millicores(total_cpu_request),
            total_memory_request: Bytes(total_memory_request),
            total_cpu_limit: total_cpu_limit.map(Millicores),
            total_memory_limit: total_memory_limit.map(Bytes),
            extended_resource_requests: extended_requests,
            extended_resource_limits: extended_limits,
        }
    }
}

/// Quality of Service (QoS) class for pods.
///
/// QoS classes determine pod eviction priority under node resource pressure:
///
/// - **Guaranteed**: Pods with requests == limits for all containers and all resources.
///   These pods receive the highest priority and are last to be evicted.
///
/// - **Burstable**: Pods with requests < limits or only some resources specified.
///   These pods have medium priority and can burst above requests up to limits.
///
/// - **BestEffort**: Pods with no requests or limits specified. These are first to be
///   evicted under pressure but can use all available node resources when idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QoSClass {
    /// Highest priority, requests == limits for all resources.
    Guaranteed,
    /// Medium priority, requests < limits or partial specifications.
    Burstable,
    /// Lowest priority, no requests or limits.
    BestEffort,
}

/// Trait for types that have resource capacity.
pub trait ResourceCapacity {
    /// Returns the total CPU capacity in millicores.
    fn cpu_capacity(&self) -> Millicores;

    /// Returns the total memory capacity in bytes.
    fn memory_capacity(&self) -> Bytes;

    /// Returns the total disk capacity in bytes.
    fn disk_capacity(&self) -> Bytes;
}

impl ResourceCapacity for NodeResources {
    fn cpu_capacity(&self) -> Millicores {
        self.cpu_capacity
    }

    fn memory_capacity(&self) -> Bytes {
        self.memory_capacity
    }

    fn disk_capacity(&self) -> Bytes {
        self.disk_capacity
    }
}

/// Trait for computing allocatable resources from capacity and reserved resources.
pub trait Allocatable {
    /// Derives allocatable resources from capacity and reserved resources.
    ///
    /// Formula: `allocatable = capacity - reserved`
    ///
    /// This ensures that system processes have guaranteed resources while
    /// allowing the scheduler to safely allocate the remainder to pods.
    fn derive_from_capacity(
        capacity: &NodeResources,
        reserved: &ReservedResources,
    ) -> AllocatableResources;
}

impl Allocatable for AllocatableResources {
    fn derive_from_capacity(
        capacity: &NodeResources,
        reserved: &ReservedResources,
    ) -> AllocatableResources {
        AllocatableResources {
            cpu_allocatable: Millicores(
                capacity
                    .cpu_capacity
                    .0
                    .saturating_sub(reserved.cpu_reserved.0),
            ),
            memory_allocatable: Bytes(
                capacity
                    .memory_capacity
                    .0
                    .saturating_sub(reserved.memory_reserved.0),
            ),
            disk_allocatable: Bytes(
                capacity
                    .disk_capacity
                    .0
                    .saturating_sub(reserved.disk_reserved.0),
            ),
            gpu_allocatable: capacity.gpu_capacity,
            ephemeral_storage_allocatable: capacity.ephemeral_storage_capacity,
            hugepages_allocatable: capacity.hugepages_capacity,
            extended_resources: capacity.extended_resources.clone(),
        }
    }
}

/// Trait for deriving QoS class from container resource specifications.
pub trait QoSClassification {
    /// Derives the QoS class from container resources within a pod.
    ///
    /// Rules:
    /// - **BestEffort**: No requests or limits for any resource
    /// - **Guaranteed**: All containers have requests == limits for CPU and memory
    /// - **Burstable**: Everything else (requests < limits or partial specs)
    fn derive_qos_class(containers: &[ContainerResources]) -> QoSClass;
}

impl QoSClassification for QoSClass {
    fn derive_qos_class(containers: &[ContainerResources]) -> QoSClass {
        if containers.is_empty() {
            return QoSClass::BestEffort;
        }

        let mut has_any_request_or_limit = false;
        let mut all_guaranteed = true;

        for container in containers {
            // Check if this container has any requests or limits
            let has_specs = container.cpu_request.is_some()
                || container.memory_request.is_some()
                || container.cpu_limit.is_some()
                || container.memory_limit.is_some()
                || !container.extended_resource_requests.is_empty()
                || !container.extended_resource_limits.is_empty();

            if has_specs {
                has_any_request_or_limit = true;
            }

            // For Guaranteed, must have CPU and memory requests == limits
            let cpu_guaranteed = match (container.cpu_request, container.cpu_limit) {
                (Some(req), Some(lim)) => req == lim,
                (None, None) => false, // Must have both for Guaranteed
                _ => false,
            };

            let memory_guaranteed = match (container.memory_request, container.memory_limit) {
                (Some(req), Some(lim)) => req == lim,
                (None, None) => false, // Must have both for Guaranteed
                _ => false,
            };

            // Extended resources must be equal if present
            let extended_guaranteed =
                container.extended_resource_requests == container.extended_resource_limits;

            if !cpu_guaranteed || !memory_guaranteed || !extended_guaranteed {
                all_guaranteed = false;
            }
        }

        if !has_any_request_or_limit {
            QoSClass::BestEffort
        } else if all_guaranteed {
            QoSClass::Guaranteed
        } else {
            QoSClass::Burstable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_millicores_conversion() {
        let cores = Millicores::from_cores(2);
        assert_eq!(cores.0, 2000);
        assert_eq!(cores.to_cores_f64(), 2.0);

        let half_core = Millicores(500);
        assert_eq!(half_core.to_cores_f64(), 0.5);
    }

    #[test]
    fn test_bytes_conversion() {
        assert_eq!(Bytes::from_kibibytes(1).0, 1024);
        assert_eq!(Bytes::from_mebibytes(1).0, 1024 * 1024);
        assert_eq!(Bytes::from_gibibytes(2).0, 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_allocatable_derivation() {
        let capacity = NodeResources {
            cpu_capacity: Millicores(8000),
            memory_capacity: Bytes::from_gibibytes(16),
            disk_capacity: Bytes::from_gibibytes(100),
            gpu_capacity: Units(2),
            ephemeral_storage_capacity: Bytes::from_gibibytes(50),
            hugepages_capacity: Bytes::from_gibibytes(2),
            extended_resources: vec![],
        };

        let reserved = ReservedResources {
            cpu_reserved: Millicores(1000),
            memory_reserved: Bytes::from_gibibytes(2),
            disk_reserved: Bytes::from_gibibytes(10),
        };

        let allocatable = AllocatableResources::derive_from_capacity(&capacity, &reserved);

        assert_eq!(allocatable.cpu_allocatable.0, 7000);
        assert_eq!(
            allocatable.memory_allocatable.0,
            Bytes::from_gibibytes(14).0
        );
        assert_eq!(allocatable.disk_allocatable.0, Bytes::from_gibibytes(90).0);
    }

    #[test]
    fn test_container_validation() {
        let mut valid = ContainerResources::empty();
        valid.cpu_request = Some(Millicores(500));
        valid.cpu_limit = Some(Millicores(1000));
        assert!(valid.validate().is_none());

        let mut invalid = ContainerResources::empty();
        invalid.cpu_request = Some(Millicores(1000));
        invalid.cpu_limit = Some(Millicores(500));
        assert!(invalid.validate().is_some());
    }

    #[test]
    fn test_pod_resource_aggregation() {
        let containers = vec![
            ContainerResources {
                cpu_request: Some(Millicores(500)),
                memory_request: Some(Bytes::from_mebibytes(256)),
                cpu_limit: Some(Millicores(1000)),
                memory_limit: Some(Bytes::from_mebibytes(512)),
                ..ContainerResources::empty()
            },
            ContainerResources {
                cpu_request: Some(Millicores(250)),
                memory_request: Some(Bytes::from_mebibytes(128)),
                cpu_limit: Some(Millicores(500)),
                memory_limit: Some(Bytes::from_mebibytes(256)),
                ..ContainerResources::empty()
            },
        ];

        let pod = PodResources::from_containers(&containers);

        assert_eq!(pod.total_cpu_request.0, 750);
        assert_eq!(pod.total_memory_request.0, Bytes::from_mebibytes(384).0);
        assert_eq!(pod.total_cpu_limit, Some(Millicores(1500)));
        assert_eq!(pod.total_memory_limit, Some(Bytes::from_mebibytes(768)));
    }

    #[test]
    fn test_qos_best_effort() {
        let containers = vec![ContainerResources::empty()];
        assert_eq!(
            QoSClass::derive_qos_class(&containers),
            QoSClass::BestEffort
        );
    }

    #[test]
    fn test_qos_guaranteed() {
        let containers = vec![ContainerResources {
            cpu_request: Some(Millicores(1000)),
            cpu_limit: Some(Millicores(1000)),
            memory_request: Some(Bytes::from_gibibytes(1)),
            memory_limit: Some(Bytes::from_gibibytes(1)),
            ..ContainerResources::empty()
        }];

        assert_eq!(
            QoSClass::derive_qos_class(&containers),
            QoSClass::Guaranteed
        );
    }

    #[test]
    fn test_qos_burstable() {
        let containers = vec![ContainerResources {
            cpu_request: Some(Millicores(500)),
            cpu_limit: Some(Millicores(1000)),
            memory_request: Some(Bytes::from_gibibytes(1)),
            memory_limit: Some(Bytes::from_gibibytes(2)),
            ..ContainerResources::empty()
        }];

        assert_eq!(QoSClass::derive_qos_class(&containers), QoSClass::Burstable);
    }
}
