output "instance_id" {
  description = "GCP instance ID"
  value       = google_compute_instance.univrs_node.instance_id
}

output "instance_name" {
  description = "GCP instance name"
  value       = google_compute_instance.univrs_node.name
}

output "private_ip" {
  description = "Private IP address"
  value       = google_compute_instance.univrs_node.network_interface[0].network_ip
}

output "public_ip" {
  description = "Public IP address (static)"
  value       = google_compute_address.univrs_node.address
}

output "zone" {
  description = "GCP zone"
  value       = google_compute_instance.univrs_node.zone
}

output "node_name" {
  description = "Node name"
  value       = var.node_name
}

output "node_role" {
  description = "Node role (bootstrap/worker)"
  value       = var.node_role
}

output "connection_strings" {
  description = "Connection strings for this node"
  value = {
    ssh               = "ssh -i <key> ${var.ssh_user}@${google_compute_address.univrs_node.address}"
    network_api       = "http://${google_compute_address.univrs_node.address}:8080"
    network_ws        = "ws://${google_compute_address.univrs_node.address}:8080/ws"
    network_p2p       = "/ip4/${google_compute_address.univrs_node.address}/tcp/9000"
    orchestrator_api  = "http://${google_compute_address.univrs_node.address}:9090"
    orchestrator_seed = "${google_compute_address.univrs_node.address}:7280"
  }
}
