output "vm_id" {
  description = "Azure VM ID"
  value       = azurerm_linux_virtual_machine.univrs_node.id
}

output "private_ip" {
  description = "Private IP address of the VM"
  value       = azurerm_network_interface.univrs_node.private_ip_address
}

output "public_ip" {
  description = "Public IP address"
  value       = azurerm_public_ip.univrs_node.ip_address
}

output "network_security_group_id" {
  description = "Network Security Group ID"
  value       = azurerm_network_security_group.univrs_node.id
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
    ssh               = "ssh -i <key> ${var.admin_username}@${azurerm_public_ip.univrs_node.ip_address}"
    network_api       = "http://${azurerm_public_ip.univrs_node.ip_address}:8080"
    network_ws        = "ws://${azurerm_public_ip.univrs_node.ip_address}:8080/ws"
    network_p2p       = "/ip4/${azurerm_public_ip.univrs_node.ip_address}/tcp/9000"
    orchestrator_api  = "http://${azurerm_public_ip.univrs_node.ip_address}:9090"
    orchestrator_seed = "${azurerm_public_ip.univrs_node.ip_address}:7280"
  }
}
