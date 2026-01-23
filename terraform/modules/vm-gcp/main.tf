terraform {
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
  }
}

# Static external IP
resource "google_compute_address" "univrs_node" {
  name   = "univrs-${var.node_name}-ip"
  region = var.region
}

# Compute Instance - e2-micro (free tier eligible)
resource "google_compute_instance" "univrs_node" {
  name         = "univrs-${var.node_name}"
  machine_type = var.machine_type
  zone         = var.zone

  boot_disk {
    initialize_params {
      image = "ubuntu-os-cloud/ubuntu-2204-lts"
      size  = 30 # Free tier allows 30GB
      type  = "pd-standard"
    }
  }

  network_interface {
    network    = var.network
    subnetwork = var.subnetwork != "" ? var.subnetwork : null

    access_config {
      nat_ip = google_compute_address.univrs_node.address
    }
  }

  metadata = {
    ssh-keys = "${var.ssh_user}:${var.ssh_public_key}"
  }

  metadata_startup_script = templatefile("${path.module}/startup-script.sh", {
    node_role                 = var.node_role
    node_name                 = var.node_name
    bootstrap_ip              = var.bootstrap_ip
    cluster_id                = var.cluster_id
    docker_image_network      = var.docker_image_network
    docker_image_orchestrator = var.docker_image_orchestrator
  })

  tags = ["univrs-node", var.node_role]

  labels = merge(var.labels, {
    environment = var.environment
    role        = var.node_role
    project     = "univrs-io"
    managed-by  = "terraform"
  })

  service_account {
    scopes = ["cloud-platform"]
  }

  allow_stopping_for_update = true
}
