# Univrs.io CI/CD & Infrastructure Guide

> A comprehensive guide for developers to understand and deploy the multi-cloud ecosystem.

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Prerequisites](#prerequisites)
4. [Phase 1: Terraform Infrastructure](#phase-1-terraform-infrastructure)
5. [Phase 2: GitHub Actions CI/CD](#phase-2-github-actions-cicd)
6. [Phase 3: DOL Ontology](#phase-3-dol-ontology)
7. [Deployment Workflow](#deployment-workflow)
8. [PR Checklist](#pr-checklist)
9. [Troubleshooting](#troubleshooting)

---

## Overview

This guide covers three interconnected systems:

| Component | Repository | Purpose |
|-----------|------------|---------|
| Terraform IaC | `univrs-orchestration` | Multi-cloud VM provisioning |
| GitHub Actions | `univrs-orchestration` | CI/CD pipelines |
| DOL Ontology | `metalearn.org` | Formal type specification |

### What Gets Deployed

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Univrs.io Production Cluster                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐           │
│  │  Oracle Cloud   │   │     Azure       │   │      AWS        │           │
│  │  (Bootstrap)    │   │    (Worker)     │   │    (Worker)     │           │
│  │                 │   │                 │   │                 │           │
│  │  A1.Flex ARM    │◄─►│  B1s            │◄─►│  t2.micro       │           │
│  │  2 OCPU/12GB    │   │  1vCPU/1GB      │   │  1vCPU/1GB      │           │
│  │  Always Free    │   │  Free Tier      │   │  Free Tier      │           │
│  └────────┬────────┘   └────────┬────────┘   └────────┬────────┘           │
│           │                     │                     │                     │
│           │         Chitchat Gossip (UDP 7280)        │                     │
│           │         libp2p P2P (TCP/UDP 9000)         │                     │
│           │                     │                     │                     │
│           │            ┌────────┴────────┐            │                     │
│           │            │      GCP        │            │                     │
│           └───────────►│    (Worker)     │◄───────────┘                     │
│                        │                 │                                  │
│                        │  e2-micro       │                                  │
│                        │  Free Tier      │                                  │
│                        └─────────────────┘                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

Each node runs two containers:
- **univrs-orchestrator** - Container orchestration (chitchat gossip, REST API)
- **univrs-network** - P2P networking (libp2p, gossipsub)

---

## Architecture

### Port Matrix

| Port | Protocol | Service | Access |
|------|----------|---------|--------|
| 22 | TCP | SSH | Admin IPs only |
| 80/443 | TCP | Dashboard | Public |
| 7280 | UDP | Chitchat gossip | Cluster nodes |
| 8080 | TCP | Network API/WS | Public |
| 9000-9001 | TCP/UDP | libp2p P2P | Public |
| 9090 | TCP | Orchestrator API | Public |

### CI/CD Flow

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Developer  │     │    GitHub    │     │    GHCR      │     │    Cloud     │
│   Push/PR    │────►│   Actions    │────►│   Registry   │────►│    VMs       │
└──────────────┘     └──────────────┘     └──────────────┘     └──────────────┘
                            │
                            ├── CI: lint, test, build
                            ├── CD: docker build, push
                            └── Deploy: SSH to each cloud
```

---

## Prerequisites

### Required Tools

```bash
# Terraform 1.5+
brew install terraform          # macOS
sudo apt install terraform      # Ubuntu
choco install terraform         # Windows

# Cloud CLIs
brew install awscli             # AWS
brew install azure-cli          # Azure
brew install google-cloud-sdk   # GCP
brew install oci-cli            # Oracle

# GitHub CLI (for PR creation)
brew install gh
```

### Cloud Account Setup

#### AWS
```bash
# Configure credentials
aws configure
# Or set environment variables
export AWS_ACCESS_KEY_ID="..."
export AWS_SECRET_ACCESS_KEY="..."
export AWS_DEFAULT_REGION="us-east-1"
```

#### Azure
```bash
az login
az account set --subscription "<subscription-id>"
```

#### GCP
```bash
gcloud auth login
gcloud auth application-default login
gcloud config set project "<project-id>"
```

#### Oracle Cloud
```bash
# Create API key in OCI Console
# Configure ~/.oci/config
[DEFAULT]
user=ocid1.user.oc1..xxx
fingerprint=xx:xx:xx:xx
tenancy=ocid1.tenancy.oc1..xxx
region=us-ashburn-1
key_file=~/.oci/oci_api_key.pem
```

### GitHub Secrets

Navigate to **Settings → Secrets and variables → Actions** and add:

| Secret | Description |
|--------|-------------|
| `ORACLE_HOST` | Oracle bootstrap node public IP |
| `ORACLE_USER` | SSH username (ubuntu) |
| `ORACLE_SSH_KEY` | Private SSH key for Oracle |
| `AZURE_HOST` | Azure worker node public IP |
| `AZURE_USER` | SSH username (azureuser) |
| `AZURE_SSH_KEY` | Private SSH key for Azure |
| `AWS_HOST` | AWS worker node public IP |
| `AWS_USER` | SSH username (ec2-user) |
| `AWS_SSH_KEY` | Private SSH key for AWS |
| `GCP_HOST` | GCP worker node public IP |
| `GCP_USER` | SSH username (ubuntu) |
| `GCP_SSH_KEY` | Private SSH key for GCP |
| `CROSS_REPO_TOKEN` | PAT for cross-repo triggers (optional) |
| `SLACK_WEBHOOK_URL` | Slack notifications (optional) |

---

## Phase 1: Terraform Infrastructure

### Directory Structure

```
terraform/
├── modules/
│   ├── vm-aws/              # AWS t2.micro module
│   │   ├── main.tf          # EC2 instance + EIP
│   │   ├── variables.tf     # Input variables
│   │   ├── outputs.tf       # Instance ID, IPs
│   │   ├── security-groups.tf
│   │   └── user-data.sh     # Bootstrap script
│   │
│   ├── vm-azure/            # Azure B1s module
│   │   ├── main.tf          # Linux VM + VNet
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
│   └── vm-oracle/           # Oracle A1.Flex (ARM) module
│       ├── main.tf          # OCI instance
│       ├── variables.tf
│       ├── outputs.tf
│       ├── security-list.tf
│       └── cloud-init.yaml
│
├── environments/
│   └── production/
│       ├── main.tf          # Composes all 4 modules
│       ├── variables.tf     # Environment variables
│       ├── outputs.tf       # Cluster outputs
│       ├── backend.tf       # State storage config
│       └── terraform.tfvars.example
│
├── Makefile                 # Automation targets
└── README.md
```

### Key Concepts

#### Module Variables

Each cloud module accepts these common variables:

| Variable | Type | Description |
|----------|------|-------------|
| `node_name` | string | Unique node identifier |
| `node_role` | string | "bootstrap" or "worker" |
| `cluster_id` | string | Cluster identifier |
| `bootstrap_ip` | string | Bootstrap node IP (empty for bootstrap) |
| `docker_image_*` | string | Container images to deploy |

#### Bootstrap vs Worker

The **bootstrap node** (Oracle) starts first and initializes the cluster:
- Sets `NODE_ROLE=bootstrap`
- Sets `SEED_NODES=""` (empty)
- Other nodes connect to it

**Worker nodes** (Azure, AWS, GCP) connect to the bootstrap:
- Set `NODE_ROLE=worker`
- Set `SEED_NODES=<bootstrap_ip>:7280`

### Deployment Steps

```bash
cd terraform

# 1. Copy and configure variables
cp environments/production/terraform.tfvars.example \
   environments/production/terraform.tfvars

# Edit terraform.tfvars with your cloud credentials
vim environments/production/terraform.tfvars

# 2. Initialize Terraform
make init

# 3. Validate configuration
make validate

# 4. Plan changes
make plan

# 5. Deploy bootstrap first (Oracle)
make apply-oracle

# Wait 60 seconds for bootstrap to initialize...

# 6. Deploy workers
make apply-workers

# 7. Verify deployment
make health-check
make cluster-status
```

### Makefile Targets

| Target | Description |
|--------|-------------|
| `make help` | Show all targets |
| `make init` | Initialize Terraform |
| `make plan` | Plan all changes |
| `make apply` | Apply all changes |
| `make apply-oracle` | Deploy bootstrap only |
| `make apply-workers` | Deploy all workers |
| `make health-check` | Check all node health |
| `make cluster-status` | Show cluster membership |
| `make output` | Show Terraform outputs |
| `make destroy` | Destroy all (careful!) |

### Outputs

After deployment, access outputs via:

```bash
# Human readable
make output

# JSON (for scripts)
make output-json

# Specific values
cd environments/production
terraform output bootstrap_node
terraform output all_public_ips
terraform output health_check_urls
```

---

## Phase 2: GitHub Actions CI/CD

### CI Pipeline (`.github/workflows/ci.yml`)

Triggered on:
- Push to `main` or `develop`
- Pull requests to `main`
- Manual dispatch

#### Jobs

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ rust-check  │────►│ rust-test   │────►│rust-build   │
│ (fmt+clippy)│     │ (workspace) │     │ (release)   │
└─────────────┘     └─────────────┘     └──────┬──────┘
                                               │
┌─────────────┐                                │
│docker-build │────────────────────────────────┘
│ (test only) │
└─────────────┘
```

**Key Feature**: Creates stub `Cargo.toml` for sibling dependencies (`univrs-identity`, `univrs-state`) so CI can run without those repos.

### CD Pipeline (`.github/workflows/cd.yml`)

Triggered on:
- Push to `main`
- Git tags (`v*`)
- Manual dispatch with target selection

#### Jobs

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ build-images │────►│deploy-bootstrap──►│deploy-workers│
│ (multi-arch) │     │   (Oracle)   │     │(Azure/AWS/GCP)
└──────────────┘     └──────────────┘     └──────┬───────┘
                                                 │
┌──────────────┐     ┌──────────────┐            │
│    notify    │◄────│verify-deploy │◄───────────┘
│  (Slack)     │     │(health check)│
└──────────────┘     └──────────────┘
```

#### Deployment Strategy

1. **Build**: Multi-arch Docker image (amd64 + arm64)
2. **Push**: To GitHub Container Registry (ghcr.io)
3. **Deploy Bootstrap**: Oracle first (must be healthy before workers)
4. **Deploy Workers**: Azure, AWS, GCP in parallel
5. **Verify**: Health checks on all nodes
6. **Notify**: Slack webhook (optional)

#### Manual Deployment

```bash
# Deploy everything
gh workflow run cd.yml -f deploy_target=all

# Deploy only bootstrap
gh workflow run cd.yml -f deploy_target=bootstrap

# Deploy only workers
gh workflow run cd.yml -f deploy_target=workers
```

### Sibling Dependency Handling

The CI creates stub manifests for missing repos:

```yaml
- name: Create stub sibling dependencies
  run: |
    mkdir -p ../univrs-identity/src
    cat > ../univrs-identity/Cargo.toml << 'EOF'
    [package]
    name = "univrs-identity"
    version = "0.1.0"
    edition = "2021"
    # ... minimal implementation
    EOF
```

This allows the CI to build without requiring all repos to be present.

---

## Phase 3: DOL Ontology

### What is DOL?

**Domain Ontology Language (DOL)** is a formal specification language for the Univrs.io type system. It defines:

- **Types**: Data structures with constraints
- **Traits**: Interfaces and behaviors
- **Relationships**: Cross-type connections
- **Invariants**: System-wide guarantees

### Directory Structure

```
metalearn.org/ontology/
├── core/
│   ├── primitives.dol      # Bytes, Timestamp, Score
│   ├── identity.dol        # PublicKey, PeerId, DID
│   └── traits/
│       ├── identifiable.dol
│       └── signable.dol
├── network/
│   ├── peer.dol            # Peer, ConnectionState
│   └── traits/
│       └── gossip.dol      # Topic, GossipMessage
├── orchestration/
│   ├── node.dol            # Node, NodeStatus
│   ├── workload.dol        # WorkloadDefinition, Instance
│   └── traits/
│       └── schedulable.dol
├── economics/
│   ├── credit.dol          # CreditRelationship
│   ├── reputation.dol      # Reputation (EMA)
│   └── traits/
│       └── scoreable.dol
├── assembly/
│   ├── assembly_index.dol  # AssemblyIndex, ComplexityClass
│   ├── causal_memory.dol   # EvolutionChain
│   ├── selection.dol       # FitnessScore
│   └── traits/
│       └── evolvable.dol
└── index.dol               # Master specification
```

### Assembly Index Mapping

Each DOL type has an **Assembly Index** indicating complexity:

| Type | Assembly Index | Description |
|------|----------------|-------------|
| `core.Bytes` | 0 | Primitive building block |
| `identity.PublicKey` | 1 | Single dependency |
| `network.Peer` | 2 | Multiple dependencies |
| `orchestration.Node` | 3 | Complex composition |
| `economics.CreditRelationship` | 4 | Cross-domain |
| `assembly.EvolutionChain` | 6 | Highest complexity |

### DOL Syntax Overview

```dol
-- Type definition
gen identity.PublicKey {
    has bytes: Bytes32

    constraint valid_ed25519 {
        is_on_curve(this.bytes)
    }

    docs {
        Ed25519 public key for cryptographic identity.
    }
}

-- Trait definition
trait core.Identifiable {
    fun id() -> PublicKey
    fun peer_id() -> PeerId

    constraint each_entity_has_unique_id {
        for all a, b in Self:
            a.id() == b.id() implies a == b
    }
}

-- Implementation
impl core.Identifiable for network.Peer {
    fun id() -> PublicKey {
        this.info.public_key
    }
}
```

---

## Deployment Workflow

### Initial Setup (One-Time)

```bash
# 1. Clone repositories
git clone https://github.com/univrs/univrs-orchestration.git
git clone https://github.com/univrs/metalearn.org.git

# 2. Configure cloud credentials
# (See Prerequisites section)

# 3. Deploy infrastructure
cd univrs-orchestration/terraform
make init
make deploy-all

# 4. Configure GitHub secrets with IPs from:
make output-json
```

### Ongoing Development

```bash
# Make code changes
git checkout -b feature/my-feature

# Run local tests
cd orchestrator
cargo test --workspace

# Push and create PR
git push -u origin feature/my-feature
gh pr create --title "My feature" --body "Description"

# CI runs automatically
# CD runs on merge to main
```

### Rollback

```bash
# Rollback to previous image tag
gh workflow run cd.yml -f deploy_target=all

# Or SSH and rollback manually
ssh ubuntu@<node-ip>
docker pull ghcr.io/univrs/orchestrator-node:sha-<previous>
docker stop univrs-orchestrator
docker rm univrs-orchestrator
docker run -d --name univrs-orchestrator ... ghcr.io/univrs/orchestrator-node:sha-<previous>
```

---

## PR Checklist

### Before Creating PRs

- [ ] All Terraform files validated: `make validate`
- [ ] Terraform plan shows expected changes: `make plan`
- [ ] CI workflow syntax valid: `gh workflow view ci.yml`
- [ ] CD workflow syntax valid: `gh workflow view cd.yml`
- [ ] DOL files have consistent naming
- [ ] README updated if behavior changed

### PR Structure (Recommended)

Create **separate PRs** for reviewability:

#### PR 1: Terraform Infrastructure
```bash
cd univrs-orchestration
git checkout -b feature/terraform-multicloud
git add terraform/
git commit -m "feat: add multi-cloud Terraform infrastructure

- Add AWS t2.micro module (free tier)
- Add Azure B1s module (free tier)
- Add GCP e2-micro module (free tier)
- Add Oracle A1.Flex ARM module (always free)
- Add production environment composition
- Add Makefile with deployment automation"
git push -u origin feature/terraform-multicloud
```

#### PR 2: GitHub Actions CI/CD
```bash
git checkout main
git checkout -b feature/github-actions-cicd
git add .github/
git add CICD-Guide.md
git commit -m "feat: add GitHub Actions CI/CD pipelines

- Add CI workflow (lint, test, build, docker)
- Add CD workflow (build, deploy, verify, notify)
- Add CICD-Guide.md for developer documentation"
git push -u origin feature/github-actions-cicd
```

#### PR 3: DOL Ontology
```bash
cd ../metalearn.org
git checkout -b feature/dol-ontology
git add ontology/
git commit -m "feat: add DOL ontology specification

- Add core primitives and identity types
- Add network peer and gossip types
- Add orchestration node and workload types
- Add economics credit and reputation types
- Add Assembly Theory types
- Add master index with relationships"
git push -u origin feature/dol-ontology
```

### PR Review Focus Areas

| PR | Review Focus |
|----|--------------|
| Terraform | Security groups, instance sizes, user-data scripts |
| CI/CD | Secret handling, deployment order, rollback capability |
| DOL | Type correctness, constraint validity, documentation |

---

## Troubleshooting

### Terraform Issues

#### "Provider authentication failed"
```bash
# AWS
aws sts get-caller-identity  # Verify credentials

# Azure
az account show  # Verify logged in

# GCP
gcloud auth list  # Verify credentials

# Oracle
oci iam user get --user-id <your-user-ocid>
```

#### "Resource already exists"
```bash
# Import existing resource
terraform import module.aws_worker.aws_instance.univrs_node i-xxxx

# Or destroy and recreate
make destroy-aws
make apply-aws
```

### CI/CD Issues

#### "Sibling dependency not found"
The CI creates stubs, but if you see this:
```bash
# Check the stub creation step in workflow logs
# Ensure the paths match your workspace structure
```

#### "SSH connection refused"
```bash
# Verify node is running
curl http://<node-ip>:9090/health

# Check security group allows port 22
# Verify SSH key matches
ssh -i key.pem -v ubuntu@<node-ip>
```

#### "Health check failed after deploy"
```bash
# SSH to node and check logs
ssh ubuntu@<node-ip>
docker logs univrs-orchestrator --tail 100
docker logs univrs-network --tail 100

# Check cloud-init completed
cat /var/log/cloud-init-output.log
```

### Cluster Issues

#### "Node not joining cluster"
```bash
# Check bootstrap is reachable
curl http://<bootstrap-ip>:9090/health
curl http://<bootstrap-ip>:9090/api/v1/cluster/nodes

# Check gossip port (UDP 7280)
nc -vzu <bootstrap-ip> 7280

# Check worker seed nodes config
ssh ubuntu@<worker-ip>
docker inspect univrs-orchestrator | jq '.[0].Config.Env'
```

#### "Cluster shows fewer nodes than expected"
```bash
# Wait for gossip convergence (30-60 seconds)
sleep 60
make cluster-status

# Check each node's view
for ip in $ORACLE_IP $AZURE_IP $AWS_IP $GCP_IP; do
  echo "=== $ip ==="
  curl -s "http://$ip:9090/api/v1/cluster/nodes" | jq '.nodes | length'
done
```

---

## Cost Summary

| Provider | Instance | Free Tier | After Free Tier |
|----------|----------|-----------|-----------------|
| Oracle | A1.Flex (4 OCPU, 24GB) | **Always Free** | $0 |
| Azure | B1s (1 vCPU, 1GB) | 12 months | ~$8/month |
| AWS | t2.micro (1 vCPU, 1GB) | 12 months | ~$9/month |
| GCP | e2-micro | Free tier* | ~$5/month |

**Total during free tier: $0/month**

*GCP free tier has usage limits (specific regions, egress caps)

---

## Next Steps

After infrastructure is deployed:

1. **Configure monitoring**: Add Prometheus/Grafana
2. **Set up alerts**: CloudWatch, Azure Monitor, GCP Monitoring
3. **Enable backups**: Snapshot EBS/disk volumes
4. **Add TLS**: Let's Encrypt certificates for HTTPS
5. **Integrate network**: Deploy univrs-network containers

---

## Support

- **Issues**: https://github.com/univrs/univrs-orchestration/issues
- **Discussions**: https://github.com/univrs/univrs-orchestration/discussions
- **Documentation**: https://docs.univrs.io
