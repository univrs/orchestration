# Univrs.io Multi-Cloud Infrastructure

This directory contains Terraform modules for deploying Univrs.io across multiple cloud providers.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Univrs.io Multi-Cloud Cluster                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────────┐        ┌───────────────────┐                        │
│  │  Oracle Cloud     │        │  Azure            │                        │
│  │  (Bootstrap)      │        │  (Worker)         │                        │
│  │                   │        │                   │                        │
│  │  A1.Flex ARM      │◄──────►│  B1s             │                        │
│  │  4 OCPU / 24GB    │        │  1 vCPU / 1GB    │                        │
│  │  Always Free      │        │  Free Tier       │                        │
│  └───────────────────┘        └───────────────────┘                        │
│           │                            │                                    │
│           │         Chitchat           │                                    │
│           │         Gossip             │                                    │
│           │         (UDP 7280)         │                                    │
│           │                            │                                    │
│  ┌────────┴────────┐        ┌──────────┴─────────┐                         │
│  │  AWS            │        │  GCP               │                         │
│  │  (Worker)       │        │  (Worker)          │                         │
│  │                 │        │                    │                         │
│  │  t2.micro       │◄──────►│  e2-micro          │                         │
│  │  1 vCPU / 1GB   │        │  0.25-2 vCPU       │                         │
│  │  Free Tier      │        │  Free Tier         │                         │
│  └─────────────────┘        └────────────────────┘                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Port Matrix

| Port | Protocol | Service | Description |
|------|----------|---------|-------------|
| 22 | TCP | SSH | Admin access (restricted) |
| 80 | TCP | HTTP | Dashboard (future) |
| 443 | TCP | HTTPS | Dashboard TLS (future) |
| 7280 | UDP | Chitchat | Orchestrator gossip protocol |
| 8080 | TCP | HTTP/WS | Network REST API & WebSocket |
| 9000-9001 | TCP/UDP | libp2p | P2P transport (TCP & QUIC) |
| 9090 | TCP | HTTP | Orchestrator REST API |

## Prerequisites

### Required Tools

```bash
# Terraform 1.5+
brew install terraform  # macOS
# or
sudo apt install terraform  # Ubuntu

# Cloud CLIs
brew install awscli azure-cli google-cloud-sdk oci-cli
```

### Cloud Account Setup

1. **AWS**: Create IAM user with EC2 permissions, configure `~/.aws/credentials`
2. **Azure**: Run `az login` and `az account set --subscription <id>`
3. **GCP**: Run `gcloud auth application-default login`
4. **Oracle**: Create API key in OCI console, configure `~/.oci/config`

## Quick Start

```bash
# 1. Initialize
make init

# 2. Copy and configure variables
cp environments/production/terraform.tfvars.example environments/production/terraform.tfvars
# Edit terraform.tfvars with your cloud credentials

# 3. Validate configuration
make validate

# 4. Deploy (recommended order)
make apply-oracle       # Bootstrap first
make apply-workers      # Then workers

# 5. Verify
make health-check
make cluster-status
```

## Directory Structure

```
terraform/
├── modules/
│   ├── vm-aws/              # AWS t2.micro module
│   │   ├── main.tf          # EC2 instance
│   │   ├── variables.tf     # Input variables
│   │   ├── outputs.tf       # Output values
│   │   ├── security-groups.tf
│   │   └── user-data.sh     # Bootstrap script
│   │
│   ├── vm-azure/            # Azure B1s module
│   │   ├── main.tf          # Linux VM
│   │   ├── variables.tf
│   │   ├── outputs.tf
│   │   ├── network-security-group.tf
│   │   └── cloud-init.yaml
│   │
│   ├── vm-gcp/              # GCP e2-micro module
│   │   ├── main.tf          # Compute instance
│   │   ├── variables.tf
│   │   ├── outputs.tf
│   │   ├── firewall.tf
│   │   └── startup-script.sh
│   │
│   └── vm-oracle/           # Oracle A1.Flex ARM module
│       ├── main.tf          # OCI instance
│       ├── variables.tf
│       ├── outputs.tf
│       ├── security-list.tf
│       └── cloud-init.yaml
│
├── environments/
│   └── production/
│       ├── main.tf          # Module composition
│       ├── variables.tf     # Environment vars
│       ├── outputs.tf       # Cluster outputs
│       ├── backend.tf       # State storage
│       └── terraform.tfvars.example
│
├── Makefile                 # Automation targets
└── README.md               # This file
```

## Make Targets

```bash
make help              # Show all targets

# Core
make init              # Initialize Terraform
make plan              # Plan changes
make apply             # Apply all changes
make destroy           # Destroy everything (careful!)

# Per-Cloud
make apply-oracle      # Deploy bootstrap node
make apply-azure       # Deploy Azure worker
make apply-aws         # Deploy AWS worker
make apply-gcp         # Deploy GCP worker
make apply-workers     # Deploy all workers

# Operations
make health-check      # Check all node health
make cluster-status    # Show cluster membership
make output            # Show Terraform outputs
make ssh-oracle        # SSH to bootstrap node
make logs-oracle       # View bootstrap logs

# Full Workflow
make deploy-all        # Complete deployment
```

## Deployment Order

The bootstrap node (Oracle) must be deployed first:

```bash
# 1. Deploy bootstrap
make apply-oracle

# Wait for bootstrap to be healthy (60 seconds automatically)

# 2. Deploy workers (can be parallel)
make apply-azure apply-aws apply-gcp

# 3. Verify cluster
make health-check
make cluster-status
```

## Outputs

After deployment, view all outputs:

```bash
# Human readable
make output

# JSON format (for scripts)
make output-json
```

Key outputs:
- `all_public_ips` - List of all node IPs
- `connection_info` - SSH and API connection strings
- `health_check_urls` - Health endpoints
- `cluster_membership_url` - Cluster API endpoint

## Cost

| Provider | Instance | Free Tier | After Free Tier |
|----------|----------|-----------|-----------------|
| Oracle | A1.Flex (4 OCPU, 24GB) | Always Free | $0 |
| Azure | B1s (1 vCPU, 1GB) | 12 months | ~$8/month |
| AWS | t2.micro (1 vCPU, 1GB) | 12 months | ~$9/month |
| GCP | e2-micro (0.25-2 vCPU) | Always Free* | ~$5/month |

*GCP free tier has usage limits (30GB egress, specific regions)

**Total cost during free tier: $0/month**

## Troubleshooting

### Node Not Joining Cluster

```bash
# Check bootstrap is healthy
curl http://<bootstrap-ip>:9090/health

# Check worker can reach bootstrap
ssh <worker> "curl http://<bootstrap-ip>:7280"

# Check container logs
ssh <node> "docker logs univrs-orchestrator"
```

### SSH Connection Refused

1. Check security group/NSG allows port 22
2. Verify SSH key matches
3. Check instance is running: `terraform state list`

### Container Not Starting

```bash
# Check Docker status
ssh <node> "systemctl status docker"

# Check cloud-init logs
ssh <node> "cat /var/log/cloud-init-output.log"
```

## Security Considerations

1. **SSH Access**: Restrict `admin_cidr_blocks` to known IPs
2. **API Endpoints**: Currently open; add auth for production
3. **State File**: Enable S3 backend with encryption for production
4. **Secrets**: Use Terraform Cloud or Vault for sensitive values

## Related Documentation

- [Orchestrator Architecture](../docs/ARCHITECTURE.md)
- [Cluster Management](../orchestrator/cluster_manager/README.md)
- [Network Layer](../../univrs-network/README.md)
