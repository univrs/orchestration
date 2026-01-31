variable "node_name" {
  description = "Name for this node (used in resource naming)"
  type        = string
}

variable "node_role" {
  description = "Role of this node: bootstrap or worker"
  type        = string
  validation {
    condition     = contains(["bootstrap", "worker"], var.node_role)
    error_message = "node_role must be 'bootstrap' or 'worker'"
  }
}

variable "environment" {
  description = "Environment name (production, staging)"
  type        = string
  default     = "production"
}

variable "cluster_id" {
  description = "Cluster identifier for gossip protocol"
  type        = string
}

variable "bootstrap_ip" {
  description = "IP address of bootstrap node (empty for bootstrap itself)"
  type        = string
  default     = ""
}

# OCI-specific variables
variable "compartment_id" {
  description = "OCI compartment OCID"
  type        = string
}

variable "availability_domain" {
  description = "OCI availability domain"
  type        = string
}

variable "subnet_id" {
  description = "Subnet OCID"
  type        = string
}

variable "ssh_public_key" {
  description = "SSH public key for instance access"
  type        = string
}

variable "ssh_user" {
  description = "SSH username (ubuntu for Ubuntu images)"
  type        = string
  default     = "ubuntu"
}

# A1.Flex (ARM) - Always Free: 4 OCPUs, 24GB RAM total per account
# We use 2 OCPUs, 12GB RAM per instance (can have 2 instances)
variable "ocpus" {
  description = "Number of OCPUs (ARM cores)"
  type        = number
  default     = 2
}

variable "memory_in_gbs" {
  description = "Memory in GB"
  type        = number
  default     = 12
}

# Docker images
variable "docker_image_network" {
  description = "Docker image for univrs-network (must support ARM64)"
  type        = string
  default     = "ghcr.io/univrs/mycelial-node:latest"
}

variable "docker_image_orchestrator" {
  description = "Docker image for univrs-orchestration (must support ARM64)"
  type        = string
  default     = "ghcr.io/univrs/orchestrator-node:latest"
}

variable "freeform_tags" {
  description = "Additional freeform tags to apply"
  type        = map(string)
  default     = {}
}
