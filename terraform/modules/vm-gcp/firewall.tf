# Firewall rules for Univrs.io nodes
resource "google_compute_firewall" "univrs_ssh" {
  name    = "univrs-${var.environment}-ssh"
  network = var.network

  allow {
    protocol = "tcp"
    ports    = ["22"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["univrs-node"]
  description   = "SSH access to Univrs.io nodes"
}

resource "google_compute_firewall" "univrs_http" {
  name    = "univrs-${var.environment}-http"
  network = var.network

  allow {
    protocol = "tcp"
    ports    = ["80", "443"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["univrs-node"]
  description   = "HTTP/HTTPS dashboard access"
}

resource "google_compute_firewall" "univrs_network_api" {
  name    = "univrs-${var.environment}-network-api"
  network = var.network

  allow {
    protocol = "tcp"
    ports    = ["8080"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["univrs-node"]
  description   = "Network HTTP/WebSocket API"
}

resource "google_compute_firewall" "univrs_libp2p_tcp" {
  name    = "univrs-${var.environment}-libp2p-tcp"
  network = var.network

  allow {
    protocol = "tcp"
    ports    = ["9000", "9001"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["univrs-node"]
  description   = "libp2p TCP transport"
}

resource "google_compute_firewall" "univrs_libp2p_quic" {
  name    = "univrs-${var.environment}-libp2p-quic"
  network = var.network

  allow {
    protocol = "udp"
    ports    = ["9000", "9001"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["univrs-node"]
  description   = "libp2p QUIC transport"
}

resource "google_compute_firewall" "univrs_chitchat" {
  name    = "univrs-${var.environment}-chitchat"
  network = var.network

  allow {
    protocol = "udp"
    ports    = ["7280"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["univrs-node"]
  description   = "Chitchat gossip protocol"
}

resource "google_compute_firewall" "univrs_orchestrator" {
  name    = "univrs-${var.environment}-orchestrator"
  network = var.network

  allow {
    protocol = "tcp"
    ports    = ["9090"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["univrs-node"]
  description   = "Orchestrator REST API"
}
