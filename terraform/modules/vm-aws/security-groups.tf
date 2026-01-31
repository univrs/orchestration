# Security Group for Univrs.io Node
resource "aws_security_group" "univrs_node" {
  name        = "univrs-${var.node_name}-sg"
  description = "Security group for Univrs.io ${var.node_role} node"
  vpc_id      = var.vpc_id

  # SSH - restricted to admin IPs
  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = length(var.admin_cidr_blocks) > 0 ? var.admin_cidr_blocks : ["0.0.0.0/0"]
    description = "SSH admin access"
  }

  # HTTP - Dashboard
  ingress {
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "HTTP dashboard"
  }

  # HTTPS - Dashboard
  ingress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "HTTPS dashboard"
  }

  # univrs-network P2P HTTP/WebSocket API
  ingress {
    from_port   = 8080
    to_port     = 8080
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "Network HTTP/WS API"
  }

  # libp2p TCP
  ingress {
    from_port   = 9000
    to_port     = 9001
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "libp2p TCP"
  }

  # libp2p QUIC
  ingress {
    from_port   = 9000
    to_port     = 9001
    protocol    = "udp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "libp2p QUIC"
  }

  # Chitchat gossip - can restrict to peer nodes only
  ingress {
    from_port   = 7280
    to_port     = 7280
    protocol    = "udp"
    cidr_blocks = length(var.peer_cidr_blocks) > 0 ? var.peer_cidr_blocks : ["0.0.0.0/0"]
    description = "Chitchat gossip protocol"
  }

  # Orchestrator REST API
  ingress {
    from_port   = 9090
    to_port     = 9090
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "Orchestrator REST API"
  }

  # All outbound traffic
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
    description = "All outbound traffic"
  }

  tags = merge(var.tags, {
    Name        = "univrs-${var.node_name}-sg"
    Environment = var.environment
    Project     = "univrs-io"
  })

  lifecycle {
    create_before_destroy = true
  }
}
