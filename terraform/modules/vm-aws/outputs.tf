output "instance_id" {
  description = "EC2 instance ID"
  value       = aws_instance.univrs_node.id
}

output "private_ip" {
  description = "Private IP address of the instance"
  value       = aws_instance.univrs_node.private_ip
}

output "public_ip" {
  description = "Public IP address (Elastic IP)"
  value       = aws_eip.univrs_node.public_ip
}

output "security_group_id" {
  description = "Security group ID"
  value       = aws_security_group.univrs_node.id
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
    ssh              = "ssh -i <key> ec2-user@${aws_eip.univrs_node.public_ip}"
    network_api      = "http://${aws_eip.univrs_node.public_ip}:8080"
    network_ws       = "ws://${aws_eip.univrs_node.public_ip}:8080/ws"
    network_p2p      = "/ip4/${aws_eip.univrs_node.public_ip}/tcp/9000"
    orchestrator_api = "http://${aws_eip.univrs_node.public_ip}:9090"
    orchestrator_seed = "${aws_eip.univrs_node.public_ip}:7280"
  }
}
