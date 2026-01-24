terraform {
  required_version = ">= 1.5.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    azurerm = {
      source  = "hashicorp/azurerm"
      version = "~> 3.0"
    }
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
    oci = {
      source  = "oracle/oci"
      version = "~> 5.0"
    }
  }
}

# ==============================================================================
# Provider Configuration
# ==============================================================================

provider "aws" {
  region = var.aws_region
}

provider "azurerm" {
  features {}
}

provider "google" {
  project = var.gcp_project
  region  = var.gcp_region
}

provider "oci" {
  # Uses OCI config file or environment variables
}

# ==============================================================================
# Oracle Cloud - Bootstrap Node (Always Free ARM)
# ==============================================================================

module "oracle_bootstrap" {
  source = "../../modules/vm-oracle"
  count  = var.oracle_enabled ? 1 : 0

  node_name    = "oracle-bootstrap"
  node_role    = "bootstrap"
  environment  = var.environment
  cluster_id   = var.cluster_id
  bootstrap_ip = "" # Bootstrap doesn't need this

  compartment_id      = var.oracle_compartment_id
  availability_domain = var.oracle_availability_domain
  subnet_id           = var.oracle_subnet_id
  ssh_public_key      = var.oracle_ssh_public_key

  # A1.Flex - use 2 OCPUs / 12GB (half of always-free allowance)
  ocpus         = 2
  memory_in_gbs = 12

  docker_image_network      = var.docker_image_network
  docker_image_orchestrator = var.docker_image_orchestrator
}

# ==============================================================================
# Azure Worker Node
# ==============================================================================

module "azure_worker" {
  source = "../../modules/vm-azure"
  count  = var.azure_enabled ? 1 : 0

  node_name    = "azure-worker"
  node_role    = "worker"
  environment  = var.environment
  cluster_id   = var.cluster_id
  bootstrap_ip = var.oracle_enabled ? module.oracle_bootstrap[0].public_ip : ""

  resource_group_name = var.azure_resource_group_name
  location            = var.azure_location
  ssh_public_key      = var.azure_ssh_public_key
  admin_cidr_blocks   = var.admin_cidr_blocks

  docker_image_network      = var.docker_image_network
  docker_image_orchestrator = var.docker_image_orchestrator

  depends_on = [module.oracle_bootstrap]
}

# ==============================================================================
# AWS Worker Node
# ==============================================================================

module "aws_worker" {
  source = "../../modules/vm-aws"
  count  = var.aws_enabled ? 1 : 0

  node_name    = "aws-worker"
  node_role    = "worker"
  environment  = var.environment
  cluster_id   = var.cluster_id
  bootstrap_ip = var.oracle_enabled ? module.oracle_bootstrap[0].public_ip : ""

  vpc_id            = var.aws_vpc_id
  subnet_id         = var.aws_subnet_id
  ami_id            = var.aws_ami_id
  ssh_key_name      = var.aws_ssh_key_name
  admin_cidr_blocks = var.admin_cidr_blocks

  docker_image_network      = var.docker_image_network
  docker_image_orchestrator = var.docker_image_orchestrator

  depends_on = [module.oracle_bootstrap]
}

# ==============================================================================
# GCP Worker Node
# ==============================================================================

module "gcp_worker" {
  source = "../../modules/vm-gcp"
  count  = var.gcp_enabled ? 1 : 0

  node_name    = "gcp-worker"
  node_role    = "worker"
  environment  = var.environment
  cluster_id   = var.cluster_id
  bootstrap_ip = var.oracle_enabled ? module.oracle_bootstrap[0].public_ip : ""

  project        = var.gcp_project
  region         = var.gcp_region
  zone           = var.gcp_zone
  ssh_public_key = var.gcp_ssh_public_key

  docker_image_network      = var.docker_image_network
  docker_image_orchestrator = var.docker_image_orchestrator

  depends_on = [module.oracle_bootstrap]
}

# ==============================================================================
# Locals for peer CIDR calculation
# ==============================================================================

locals {
  all_public_ips = compact([
    var.oracle_enabled ? module.oracle_bootstrap[0].public_ip : "",
    var.azure_enabled ? module.azure_worker[0].public_ip : "",
    var.aws_enabled ? module.aws_worker[0].public_ip : "",
    var.gcp_enabled ? module.gcp_worker[0].public_ip : "",
  ])

  peer_cidr_blocks = [for ip in local.all_public_ips : "${ip}/32"]
}
