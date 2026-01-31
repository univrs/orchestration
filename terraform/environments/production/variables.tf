# ==============================================================================
# Global Variables
# ==============================================================================

variable "cluster_id" {
  description = "Unique cluster identifier for gossip protocol"
  type        = string
  default     = "univrs-production"
}

variable "environment" {
  description = "Environment name"
  type        = string
  default     = "production"
}

# ==============================================================================
# AWS Variables
# ==============================================================================

variable "aws_enabled" {
  description = "Enable AWS node deployment"
  type        = bool
  default     = true
}

variable "aws_region" {
  description = "AWS region"
  type        = string
  default     = "us-east-1"
}

variable "aws_vpc_id" {
  description = "AWS VPC ID"
  type        = string
}

variable "aws_subnet_id" {
  description = "AWS subnet ID"
  type        = string
}

variable "aws_ami_id" {
  description = "AWS AMI ID (Amazon Linux 2023 or Ubuntu 22.04)"
  type        = string
}

variable "aws_ssh_key_name" {
  description = "AWS SSH key pair name"
  type        = string
}

# ==============================================================================
# Azure Variables
# ==============================================================================

variable "azure_enabled" {
  description = "Enable Azure node deployment"
  type        = bool
  default     = true
}

variable "azure_resource_group_name" {
  description = "Azure resource group name"
  type        = string
}

variable "azure_location" {
  description = "Azure region"
  type        = string
  default     = "eastus"
}

variable "azure_ssh_public_key" {
  description = "Azure SSH public key"
  type        = string
}

# ==============================================================================
# GCP Variables
# ==============================================================================

variable "gcp_enabled" {
  description = "Enable GCP node deployment"
  type        = bool
  default     = true
}

variable "gcp_project" {
  description = "GCP project ID"
  type        = string
}

variable "gcp_region" {
  description = "GCP region"
  type        = string
  default     = "us-central1"
}

variable "gcp_zone" {
  description = "GCP zone"
  type        = string
  default     = "us-central1-a"
}

variable "gcp_ssh_public_key" {
  description = "GCP SSH public key"
  type        = string
}

# ==============================================================================
# Oracle Variables (Bootstrap node - always free ARM)
# ==============================================================================

variable "oracle_enabled" {
  description = "Enable Oracle node deployment (bootstrap)"
  type        = bool
  default     = true
}

variable "oracle_compartment_id" {
  description = "Oracle Cloud compartment OCID"
  type        = string
}

variable "oracle_availability_domain" {
  description = "Oracle Cloud availability domain"
  type        = string
}

variable "oracle_subnet_id" {
  description = "Oracle Cloud subnet OCID"
  type        = string
}

variable "oracle_ssh_public_key" {
  description = "Oracle SSH public key"
  type        = string
}

# ==============================================================================
# Docker Images
# ==============================================================================

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

# ==============================================================================
# Admin Access
# ==============================================================================

variable "admin_cidr_blocks" {
  description = "CIDR blocks for SSH admin access"
  type        = list(string)
  default     = []
}
