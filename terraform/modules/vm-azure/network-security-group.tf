# Network Security Group for Univrs.io Node
resource "azurerm_network_security_group" "univrs_node" {
  name                = "univrs-${var.node_name}-nsg"
  location            = var.location
  resource_group_name = var.resource_group_name

  # SSH - restricted to admin IPs
  security_rule {
    name                       = "SSH"
    priority                   = 100
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "22"
    source_address_prefixes    = length(var.admin_cidr_blocks) > 0 ? var.admin_cidr_blocks : ["0.0.0.0/0"]
    destination_address_prefix = "*"
  }

  # HTTP - Dashboard
  security_rule {
    name                       = "HTTP"
    priority                   = 110
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "80"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }

  # HTTPS - Dashboard
  security_rule {
    name                       = "HTTPS"
    priority                   = 120
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "443"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }

  # Network HTTP/WebSocket API
  security_rule {
    name                       = "NetworkAPI"
    priority                   = 130
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "8080"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }

  # libp2p TCP
  security_rule {
    name                       = "libp2pTCP"
    priority                   = 140
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "9000-9001"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }

  # libp2p QUIC
  security_rule {
    name                       = "libp2pQUIC"
    priority                   = 150
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Udp"
    source_port_range          = "*"
    destination_port_range     = "9000-9001"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }

  # Chitchat gossip
  security_rule {
    name                       = "ChitchatGossip"
    priority                   = 160
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Udp"
    source_port_range          = "*"
    destination_port_range     = "7280"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }

  # Orchestrator REST API
  security_rule {
    name                       = "OrchestratorAPI"
    priority                   = 170
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "9090"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }

  tags = merge(var.tags, {
    Environment = var.environment
    Project     = "univrs-io"
  })
}
