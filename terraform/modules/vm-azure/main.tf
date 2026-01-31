terraform {
  required_providers {
    azurerm = {
      source  = "hashicorp/azurerm"
      version = "~> 3.0"
    }
  }
}

# Network Interface
resource "azurerm_network_interface" "univrs_node" {
  name                = "univrs-${var.node_name}-nic"
  location            = var.location
  resource_group_name = var.resource_group_name

  ip_configuration {
    name                          = "internal"
    subnet_id                     = azurerm_subnet.univrs_node.id
    private_ip_address_allocation = "Dynamic"
    public_ip_address_id          = azurerm_public_ip.univrs_node.id
  }

  tags = merge(var.tags, {
    Environment = var.environment
    Project     = "univrs-io"
  })
}

# Virtual Network (if not existing)
resource "azurerm_virtual_network" "univrs_node" {
  name                = "univrs-${var.environment}-vnet"
  address_space       = ["10.0.0.0/16"]
  location            = var.location
  resource_group_name = var.resource_group_name

  tags = merge(var.tags, {
    Environment = var.environment
    Project     = "univrs-io"
  })
}

# Subnet
resource "azurerm_subnet" "univrs_node" {
  name                 = "univrs-${var.environment}-subnet"
  resource_group_name  = var.resource_group_name
  virtual_network_name = azurerm_virtual_network.univrs_node.name
  address_prefixes     = ["10.0.1.0/24"]
}

# Public IP
resource "azurerm_public_ip" "univrs_node" {
  name                = "univrs-${var.node_name}-pip"
  resource_group_name = var.resource_group_name
  location            = var.location
  allocation_method   = "Static"
  sku                 = "Basic"

  tags = merge(var.tags, {
    Environment = var.environment
    Project     = "univrs-io"
  })
}

# Linux Virtual Machine - Standard_B1s (free tier eligible)
resource "azurerm_linux_virtual_machine" "univrs_node" {
  name                = "univrs-${var.node_name}"
  resource_group_name = var.resource_group_name
  location            = var.location
  size                = var.vm_size
  admin_username      = var.admin_username

  network_interface_ids = [
    azurerm_network_interface.univrs_node.id,
  ]

  admin_ssh_key {
    username   = var.admin_username
    public_key = var.ssh_public_key
  }

  os_disk {
    caching              = "ReadWrite"
    storage_account_type = "Standard_LRS"
    disk_size_gb         = 30
  }

  source_image_reference {
    publisher = "Canonical"
    offer     = "0001-com-ubuntu-server-jammy"
    sku       = "22_04-lts-gen2"
    version   = "latest"
  }

  custom_data = base64encode(templatefile("${path.module}/cloud-init.yaml", {
    node_role                 = var.node_role
    node_name                 = var.node_name
    bootstrap_ip              = var.bootstrap_ip
    cluster_id                = var.cluster_id
    docker_image_network      = var.docker_image_network
    docker_image_orchestrator = var.docker_image_orchestrator
  }))

  tags = merge(var.tags, {
    Name        = "univrs-${var.node_name}"
    Environment = var.environment
    Role        = var.node_role
    Project     = "univrs-io"
    ManagedBy   = "terraform"
  })
}

# Associate NSG with NIC
resource "azurerm_network_interface_security_group_association" "univrs_node" {
  network_interface_id      = azurerm_network_interface.univrs_node.id
  network_security_group_id = azurerm_network_security_group.univrs_node.id
}
