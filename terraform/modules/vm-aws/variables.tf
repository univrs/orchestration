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

# AWS-specific variables
variable "vpc_id" {
  description = "VPC ID to deploy into"
  type        = string
}

variable "subnet_id" {
  description = "Subnet ID for the instance"
  type        = string
}

variable "ami_id" {
  description = "AMI ID (Amazon Linux 2023 or Ubuntu 22.04)"
  type        = string
}

variable "ssh_key_name" {
  description = "Name of existing SSH key pair"
  type        = string
}

variable "instance_type" {
  description = "EC2 instance type"
  type        = string
  default     = "t2.micro" # Free tier eligible
}

variable "admin_cidr_blocks" {
  description = "CIDR blocks allowed for SSH access"
  type        = list(string)
  default     = []
}

variable "peer_cidr_blocks" {
  description = "CIDR blocks of peer nodes for gossip"
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
