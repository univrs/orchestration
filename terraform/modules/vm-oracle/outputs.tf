output "instance_id" {
  description = "OCI instance OCID"
  value       = oci_core_instance.univrs_node.id
}

output "private_ip" {
  description = "Private IP address"
  value       = data.oci_core_vnic.univrs_node.private_ip_address
}

output "public_ip" {
  description = "Public IP address (reserved)"
  value       = oci_core_public_ip.univrs_node_attachment.ip_address
}

output "availability_domain" {
  description = "Availability domain"
  value       = oci_core_instance.univrs_node.availability_domain
}

output "node_name" {
  description = "Node name"
  value       = var.node_name
}

output "node_role" {
  description = "Node role (bootstrap/worker)"
  value       = var.node_role
}

output "shape" {
  description = "Instance shape"
  value       = oci_core_instance.univrs_node.shape
}

output "ocpus" {
  description = "Number of OCPUs"
  value       = var.ocpus
}

output "memory_gb" {
  description = "Memory in GB"
  value       = var.memory_in_gbs
}

output "connection_strings" {
  description = "Connection strings for this node"
  value = {
    ssh               = "ssh -i <key> ${var.ssh_user}@${oci_core_public_ip.univrs_node_attachment.ip_address}"
    network_api       = "http://${oci_core_public_ip.univrs_node_attachment.ip_address}:8080"
    network_ws        = "ws://${oci_core_public_ip.univrs_node_attachment.ip_address}:8080/ws"
    network_p2p       = "/ip4/${oci_core_public_ip.univrs_node_attachment.ip_address}/tcp/9000"
    orchestrator_api  = "http://${oci_core_public_ip.univrs_node_attachment.ip_address}:9090"
    orchestrator_seed = "${oci_core_public_ip.univrs_node_attachment.ip_address}:7280"
  }
}
