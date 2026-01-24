terraform {
  required_providers {
    oci = {
      source  = "oracle/oci"
      version = "~> 5.0"
    }
  }
}

# Get available Ampere A1 (ARM) images - Ubuntu 22.04
data "oci_core_images" "ubuntu_arm" {
  compartment_id           = var.compartment_id
  operating_system         = "Canonical Ubuntu"
  operating_system_version = "22.04"
  shape                    = "VM.Standard.A1.Flex"
  sort_by                  = "TIMECREATED"
  sort_order               = "DESC"
}

# Compute Instance - A1.Flex (ARM, Always Free)
resource "oci_core_instance" "univrs_node" {
  compartment_id      = var.compartment_id
  availability_domain = var.availability_domain
  display_name        = "univrs-${var.node_name}"
  shape               = "VM.Standard.A1.Flex"

  shape_config {
    ocpus         = var.ocpus
    memory_in_gbs = var.memory_in_gbs
  }

  source_details {
    source_type = "image"
    source_id   = data.oci_core_images.ubuntu_arm.images[0].id
    boot_volume_size_in_gbs = 50 # Always Free allows up to 200GB total
  }

  create_vnic_details {
    subnet_id                 = var.subnet_id
    assign_public_ip          = true
    display_name              = "univrs-${var.node_name}-vnic"
    hostname_label            = "univrs-${var.node_name}"
    nsg_ids                   = [oci_core_network_security_group.univrs_node.id]
  }

  metadata = {
    ssh_authorized_keys = var.ssh_public_key
    user_data = base64encode(templatefile("${path.module}/cloud-init.yaml", {
      node_role                 = var.node_role
      node_name                 = var.node_name
      bootstrap_ip              = var.bootstrap_ip
      cluster_id                = var.cluster_id
      docker_image_network      = var.docker_image_network
      docker_image_orchestrator = var.docker_image_orchestrator
    }))
  }

  freeform_tags = merge(var.freeform_tags, {
    "Environment" = var.environment
    "Role"        = var.node_role
    "Project"     = "univrs-io"
    "ManagedBy"   = "terraform"
  })

  lifecycle {
    create_before_destroy = true
  }
}

# Reserved public IP for stability
resource "oci_core_public_ip" "univrs_node" {
  compartment_id = var.compartment_id
  lifetime       = "RESERVED"
  display_name   = "univrs-${var.node_name}-ip"

  freeform_tags = merge(var.freeform_tags, {
    "Environment" = var.environment
    "Project"     = "univrs-io"
  })
}

# Attach reserved IP to VNIC
resource "oci_core_public_ip" "univrs_node_attachment" {
  compartment_id = var.compartment_id
  lifetime       = "RESERVED"
  display_name   = "univrs-${var.node_name}-attached-ip"
  private_ip_id  = data.oci_core_private_ips.univrs_node.private_ips[0].id

  freeform_tags = merge(var.freeform_tags, {
    "Environment" = var.environment
    "Project"     = "univrs-io"
  })
}

# Get the private IP of the instance for IP attachment
data "oci_core_private_ips" "univrs_node" {
  vnic_id = oci_core_instance.univrs_node.create_vnic_details[0].vnic_id

  depends_on = [oci_core_instance.univrs_node]
}

# Get the actual VNIC
data "oci_core_vnic_attachments" "univrs_node" {
  compartment_id = var.compartment_id
  instance_id    = oci_core_instance.univrs_node.id
}

data "oci_core_vnic" "univrs_node" {
  vnic_id = data.oci_core_vnic_attachments.univrs_node.vnic_attachments[0].vnic_id
}
