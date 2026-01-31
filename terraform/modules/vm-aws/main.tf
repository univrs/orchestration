terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

# EC2 Instance - t2.micro (free tier eligible)
resource "aws_instance" "univrs_node" {
  ami                    = var.ami_id
  instance_type          = var.instance_type
  key_name               = var.ssh_key_name
  vpc_security_group_ids = [aws_security_group.univrs_node.id]
  subnet_id              = var.subnet_id

  user_data = templatefile("${path.module}/user-data.sh", {
    node_role                 = var.node_role
    node_name                 = var.node_name
    bootstrap_ip              = var.bootstrap_ip
    cluster_id                = var.cluster_id
    docker_image_network      = var.docker_image_network
    docker_image_orchestrator = var.docker_image_orchestrator
  })

  root_block_device {
    volume_size           = 30 # Free tier allows up to 30GB
    volume_type           = "gp3"
    delete_on_termination = true
    encrypted             = true
  }

  metadata_options {
    http_endpoint               = "enabled"
    http_tokens                 = "required" # IMDSv2 for security
    http_put_response_hop_limit = 1
  }

  tags = merge(var.tags, {
    Name        = "univrs-${var.node_name}"
    Environment = var.environment
    Role        = var.node_role
    Project     = "univrs-io"
    ManagedBy   = "terraform"
  })

  lifecycle {
    create_before_destroy = true
  }
}

# Elastic IP for stable public address
resource "aws_eip" "univrs_node" {
  instance = aws_instance.univrs_node.id
  domain   = "vpc"

  tags = merge(var.tags, {
    Name        = "univrs-${var.node_name}-eip"
    Environment = var.environment
    Project     = "univrs-io"
  })
}
