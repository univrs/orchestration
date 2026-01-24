# Network Security Group for Univrs.io Node
resource "oci_core_network_security_group" "univrs_node" {
  compartment_id = var.compartment_id
  vcn_id         = data.oci_core_subnet.univrs_node.vcn_id
  display_name   = "univrs-${var.node_name}-nsg"

  freeform_tags = merge(var.freeform_tags, {
    "Environment" = var.environment
    "Project"     = "univrs-io"
  })
}

# Get subnet info for VCN ID
data "oci_core_subnet" "univrs_node" {
  subnet_id = var.subnet_id
}

# SSH - ingress
resource "oci_core_network_security_group_security_rule" "ssh" {
  network_security_group_id = oci_core_network_security_group.univrs_node.id
  direction                 = "INGRESS"
  protocol                  = "6" # TCP
  source                    = "0.0.0.0/0"
  source_type               = "CIDR_BLOCK"
  description               = "SSH access"

  tcp_options {
    destination_port_range {
      min = 22
      max = 22
    }
  }
}

# HTTP - ingress
resource "oci_core_network_security_group_security_rule" "http" {
  network_security_group_id = oci_core_network_security_group.univrs_node.id
  direction                 = "INGRESS"
  protocol                  = "6" # TCP
  source                    = "0.0.0.0/0"
  source_type               = "CIDR_BLOCK"
  description               = "HTTP dashboard"

  tcp_options {
    destination_port_range {
      min = 80
      max = 80
    }
  }
}

# HTTPS - ingress
resource "oci_core_network_security_group_security_rule" "https" {
  network_security_group_id = oci_core_network_security_group.univrs_node.id
  direction                 = "INGRESS"
  protocol                  = "6" # TCP
  source                    = "0.0.0.0/0"
  source_type               = "CIDR_BLOCK"
  description               = "HTTPS dashboard"

  tcp_options {
    destination_port_range {
      min = 443
      max = 443
    }
  }
}

# Network API - ingress
resource "oci_core_network_security_group_security_rule" "network_api" {
  network_security_group_id = oci_core_network_security_group.univrs_node.id
  direction                 = "INGRESS"
  protocol                  = "6" # TCP
  source                    = "0.0.0.0/0"
  source_type               = "CIDR_BLOCK"
  description               = "Network HTTP/WS API"

  tcp_options {
    destination_port_range {
      min = 8080
      max = 8080
    }
  }
}

# libp2p TCP - ingress
resource "oci_core_network_security_group_security_rule" "libp2p_tcp" {
  network_security_group_id = oci_core_network_security_group.univrs_node.id
  direction                 = "INGRESS"
  protocol                  = "6" # TCP
  source                    = "0.0.0.0/0"
  source_type               = "CIDR_BLOCK"
  description               = "libp2p TCP"

  tcp_options {
    destination_port_range {
      min = 9000
      max = 9001
    }
  }
}

# libp2p QUIC - ingress
resource "oci_core_network_security_group_security_rule" "libp2p_quic" {
  network_security_group_id = oci_core_network_security_group.univrs_node.id
  direction                 = "INGRESS"
  protocol                  = "17" # UDP
  source                    = "0.0.0.0/0"
  source_type               = "CIDR_BLOCK"
  description               = "libp2p QUIC"

  udp_options {
    destination_port_range {
      min = 9000
      max = 9001
    }
  }
}

# Chitchat gossip - ingress
resource "oci_core_network_security_group_security_rule" "chitchat" {
  network_security_group_id = oci_core_network_security_group.univrs_node.id
  direction                 = "INGRESS"
  protocol                  = "17" # UDP
  source                    = "0.0.0.0/0"
  source_type               = "CIDR_BLOCK"
  description               = "Chitchat gossip"

  udp_options {
    destination_port_range {
      min = 7280
      max = 7280
    }
  }
}

# Orchestrator API - ingress
resource "oci_core_network_security_group_security_rule" "orchestrator" {
  network_security_group_id = oci_core_network_security_group.univrs_node.id
  direction                 = "INGRESS"
  protocol                  = "6" # TCP
  source                    = "0.0.0.0/0"
  source_type               = "CIDR_BLOCK"
  description               = "Orchestrator REST API"

  tcp_options {
    destination_port_range {
      min = 9090
      max = 9090
    }
  }
}

# All egress
resource "oci_core_network_security_group_security_rule" "egress_all" {
  network_security_group_id = oci_core_network_security_group.univrs_node.id
  direction                 = "EGRESS"
  protocol                  = "all"
  destination               = "0.0.0.0/0"
  destination_type          = "CIDR_BLOCK"
  description               = "All outbound traffic"
}
