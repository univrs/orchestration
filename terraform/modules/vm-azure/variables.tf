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

# Azure-specific variables
variable "resource_group_name" {
  description = "Azure resource group name"
  type        = string
}

variable "location" {
  description = "Azure region"
  type        = string
  default     = "eastus"
}

variable "admin_username" {
  description = "Admin username for the VM"
  type        = string
  default     = "azureuser"
}

variable "ssh_public_key" {
  description = "SSH public key for authentication"
  type        = string
}

variable "vm_size" {
  description = "Azure VM size"
  type        = string
  default     = "Standard_B1s" # Free tier eligible
}

variable "admin_cidr_blocks" {
  description = "CIDR blocks allowed for SSH access"
  type        = list(string)
  default     = []
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

variable "tags" {
  description = "Additional tags to apply to resources"
  type        = map(string)
  default     = {}
}
