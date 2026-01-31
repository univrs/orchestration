# ==============================================================================
# Cluster Summary
# ==============================================================================

output "cluster_id" {
  description = "Cluster identifier"
  value       = var.cluster_id
}

output "environment" {
  description = "Environment name"
  value       = var.environment
}

output "node_count" {
  description = "Total number of deployed nodes"
  value = sum([
    var.oracle_enabled ? 1 : 0,
    var.azure_enabled ? 1 : 0,
    var.aws_enabled ? 1 : 0,
    var.gcp_enabled ? 1 : 0,
  ])
}

# ==============================================================================
# Bootstrap Node (Oracle)
# ==============================================================================

output "bootstrap_node" {
  description = "Bootstrap node details"
  value = var.oracle_enabled ? {
    cloud      = "oracle"
    name       = module.oracle_bootstrap[0].node_name
    public_ip  = module.oracle_bootstrap[0].public_ip
    private_ip = module.oracle_bootstrap[0].private_ip
    shape      = module.oracle_bootstrap[0].shape
    ocpus      = module.oracle_bootstrap[0].ocpus
    memory_gb  = module.oracle_bootstrap[0].memory_gb
  } : null
}

# ==============================================================================
# Worker Nodes
# ==============================================================================

output "worker_nodes" {
  description = "Worker node details"
  value = {
    azure = var.azure_enabled ? {
      name       = module.azure_worker[0].node_name
      public_ip  = module.azure_worker[0].public_ip
      private_ip = module.azure_worker[0].private_ip
    } : null

    aws = var.aws_enabled ? {
      name        = module.aws_worker[0].node_name
      public_ip   = module.aws_worker[0].public_ip
      private_ip  = module.aws_worker[0].private_ip
      instance_id = module.aws_worker[0].instance_id
    } : null

    gcp = var.gcp_enabled ? {
      name        = module.gcp_worker[0].node_name
      public_ip   = module.gcp_worker[0].public_ip
      private_ip  = module.gcp_worker[0].private_ip
      instance_id = module.gcp_worker[0].instance_id
    } : null
  }
}

# ==============================================================================
# All Public IPs
# ==============================================================================

output "all_public_ips" {
  description = "All node public IPs"
  value = compact([
    var.oracle_enabled ? module.oracle_bootstrap[0].public_ip : "",
    var.azure_enabled ? module.azure_worker[0].public_ip : "",
    var.aws_enabled ? module.aws_worker[0].public_ip : "",
    var.gcp_enabled ? module.gcp_worker[0].public_ip : "",
  ])
}

# ==============================================================================
# Connection Strings
# ==============================================================================

output "connection_info" {
  description = "Connection information for all nodes"
  value = {
    bootstrap = var.oracle_enabled ? module.oracle_bootstrap[0].connection_strings : null
    azure     = var.azure_enabled ? module.azure_worker[0].connection_strings : null
    aws       = var.aws_enabled ? module.aws_worker[0].connection_strings : null
    gcp       = var.gcp_enabled ? module.gcp_worker[0].connection_strings : null
  }
}

# ==============================================================================
# Health Check URLs
# ==============================================================================

output "health_check_urls" {
  description = "Health check endpoints for all nodes"
  value = {
    bootstrap_orchestrator = var.oracle_enabled ? "http://${module.oracle_bootstrap[0].public_ip}:9090/health" : null
    bootstrap_network      = var.oracle_enabled ? "http://${module.oracle_bootstrap[0].public_ip}:8080/health" : null
    azure_orchestrator     = var.azure_enabled ? "http://${module.azure_worker[0].public_ip}:9090/health" : null
    azure_network          = var.azure_enabled ? "http://${module.azure_worker[0].public_ip}:8080/health" : null
    aws_orchestrator       = var.aws_enabled ? "http://${module.aws_worker[0].public_ip}:9090/health" : null
    aws_network            = var.aws_enabled ? "http://${module.aws_worker[0].public_ip}:8080/health" : null
    gcp_orchestrator       = var.gcp_enabled ? "http://${module.gcp_worker[0].public_ip}:9090/health" : null
    gcp_network            = var.gcp_enabled ? "http://${module.gcp_worker[0].public_ip}:8080/health" : null
  }
}

# ==============================================================================
# Cluster Membership URL
# ==============================================================================

output "cluster_membership_url" {
  description = "URL to check cluster membership"
  value       = var.oracle_enabled ? "http://${module.oracle_bootstrap[0].public_ip}:9090/api/v1/cluster/nodes" : null
}

# ==============================================================================
# Seed Nodes for CLI
# ==============================================================================

output "seed_nodes" {
  description = "Seed nodes list for manual node addition"
  value       = var.oracle_enabled ? "${module.oracle_bootstrap[0].public_ip}:7280" : null
}

# ==============================================================================
# libp2p Multiaddrs
# ==============================================================================

output "libp2p_bootstrap_multiaddr" {
  description = "libp2p multiaddr for bootstrap node"
  value       = var.oracle_enabled ? "/ip4/${module.oracle_bootstrap[0].public_ip}/tcp/9000" : null
}
