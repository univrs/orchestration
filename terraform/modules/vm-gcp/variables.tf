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

# GCP-specific variables
variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region"
  type        = string
  default     = "us-central1"
}

variable "zone" {
  description = "GCP zone"
  type        = string
  default     = "us-central1-a"
}

variable "network" {
  description = "VPC network name"
  type        = string
  default     = "default"
}

variable "subnetwork" {
  description = "Subnetwork name (optional)"
  type        = string
  default     = ""
}

variable "ssh_user" {
  description = "SSH username"
  type        = string
  default     = "ubuntu"
}

variable "ssh_public_key" {
  description = "SSH public key"
  type        = string
}

variable "machine_type" {
  description = "GCP machine type"
  type        = string
  default     = "e2-micro" # Free tier eligible
}

# Docker images
variable "docker_image_network" {
  description = "Docker image for univrs-network"
  type        = string
  default     = "ghcr.io/univrs/mycelial-node:latest"
}

variable "docker_image_orchestrator" {
  description = "Docker image for univrs-orchestration"
  type        = string
  default     = "ghcr.io/univrs/orchestrator-node:latest"
}

variable "labels" {
  description = "Additional labels to apply to resources"
  type        = map(string)
  default     = {}
}
