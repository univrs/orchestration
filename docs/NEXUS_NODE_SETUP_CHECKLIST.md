# Nexus Node Setup Checklist

> Quick-start guide for standing up a Univrs.io Nexus node across Oracle, AWS, Azure, and GCP.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Univrs.io Multi-Cloud Cluster                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────────┐        ┌───────────────────┐                        │
│  │  Oracle Cloud     │        │  Azure            │                        │
│  │  (Bootstrap)      │        │  (Worker)         │                        │
│  │  A1.Flex ARM      │◄──────►│  B1s             │                        │
│  │  4 OCPU / 24GB    │        │  1 vCPU / 1GB    │                        │
│  │  Always Free      │        │  Free Tier       │                        │
│  └───────────────────┘        └───────────────────┘                        │
│           │                            │                                    │
│           │         Chitchat Gossip (UDP 7280)                             │
│           │                            │                                    │
│  ┌────────┴────────┐        ┌──────────┴─────────┐                         │
│  │  AWS            │        │  GCP               │                         │
│  │  (Worker)       │        │  (Worker)          │                         │
│  │  t2.micro       │◄──────►│  e2-micro          │                         │
│  │  1 vCPU / 1GB   │        │  0.25-2 vCPU       │                         │
│  │  Free Tier      │        │  Free Tier         │                         │
│  └─────────────────┘        └────────────────────┘                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Cost Summary

| Provider | Instance | Free Tier Duration | After Free Tier |
|----------|----------|-------------------|-----------------|
| Oracle   | A1.Flex (4 OCPU, 24GB ARM) | **Always Free** | $0 |
| Azure    | B1s (1 vCPU, 1GB) | 12 months | ~$8/month |
| AWS      | t2.micro (1 vCPU, 1GB) | 12 months | ~$9/month |
| GCP      | e2-micro (0.25-2 vCPU) | **Always Free*** | ~$5/month |

**Total during free tier: $0/month**

---

## Prerequisites Checklist

### 1. Local Tools

- [ ] **Terraform 1.5+**
  ```bash
  # macOS
  brew install terraform

  # Ubuntu/Debian
  sudo apt update && sudo apt install terraform

  # Verify
  terraform version
  ```

- [ ] **Cloud CLIs**
  ```bash
  # macOS (all at once)
  brew install awscli azure-cli google-cloud-sdk oci-cli

  # Or individually per cloud...
  ```

- [ ] **SSH Key Pair** (Ed25519 recommended)
  ```bash
  ssh-keygen -t ed25519 -C "your-email@example.com" -f ~/.ssh/univrs-nexus
  cat ~/.ssh/univrs-nexus.pub  # Use this public key for all clouds
  ```

---

## Cloud Account Setup

### 2. Oracle Cloud (Bootstrap Node) - REQUIRED FIRST

Oracle provides the most generous Always Free tier - this is your bootstrap node.

- [ ] **Create Oracle Cloud Account**
  - Go to: https://www.oracle.com/cloud/free/
  - Sign up (credit card required but won't be charged for Always Free)

- [ ] **Create Compartment**
  - Navigate: Identity & Security → Compartments
  - Create new compartment: `univrs-production`
  - Copy the **Compartment OCID**: `ocid1.compartment.oc1..xxxx`

- [ ] **Create VCN (Virtual Cloud Network)**
  - Navigate: Networking → Virtual Cloud Networks
  - Click "Start VCN Wizard" → "VCN with Internet Connectivity"
  - Name: `univrs-vcn`
  - Copy **Subnet OCID** (public subnet): `ocid1.subnet.oc1.xxx`

- [ ] **Get Availability Domain**
  - Navigate: Governance → Tenancy Details → Service Limits
  - Note your availability domain: `xxxx:US-ASHBURN-AD-1`

- [ ] **Create API Key for Terraform**
  - Navigate: Identity → Users → Your User → API Keys
  - Click "Add API Key" → Generate
  - Download private key to `~/.oci/oci_api_key.pem`
  - Copy the fingerprint

- [ ] **Configure OCI CLI**
  ```bash
  oci setup config
  # Follow prompts with your tenancy OCID, user OCID, fingerprint, key file

  # Verify
  oci iam availability-domain list
  ```

**Oracle Values to Collect:**
```
oracle_compartment_id      = "ocid1.compartment.oc1..xxxx"
oracle_availability_domain = "xxxx:US-ASHBURN-AD-1"
oracle_subnet_id           = "ocid1.subnet.oc1.iad.xxxx"
oracle_ssh_public_key      = "ssh-ed25519 AAAA..."
```

---

### 3. AWS Account Setup

- [ ] **Create AWS Account**
  - Go to: https://aws.amazon.com/free/
  - Sign up for Free Tier

- [ ] **Create IAM User for Terraform**
  - Navigate: IAM → Users → Create User
  - Name: `terraform-univrs`
  - Attach policy: `AmazonEC2FullAccess` (or custom minimal policy)
  - Create Access Key → Download credentials

- [ ] **Configure AWS CLI**
  ```bash
  aws configure
  # Enter Access Key ID, Secret Access Key, Region: us-east-1

  # Verify
  aws sts get-caller-identity
  ```

- [ ] **Get Default VPC and Subnet**
  ```bash
  # Get default VPC
  aws ec2 describe-vpcs --filters "Name=is-default,Values=true" --query 'Vpcs[0].VpcId' --output text

  # Get a public subnet
  aws ec2 describe-subnets --filters "Name=vpc-id,Values=<vpc-id>" --query 'Subnets[0].SubnetId' --output text

  # Get latest Amazon Linux 2023 AMI
  aws ec2 describe-images --owners amazon --filters "Name=name,Values=al2023-ami-*-x86_64" --query 'sort_by(Images, &CreationDate)[-1].ImageId' --output text
  ```

- [ ] **Create SSH Key Pair**
  ```bash
  aws ec2 import-key-pair --key-name univrs-nexus --public-key-material fileb://~/.ssh/univrs-nexus.pub
  ```

**AWS Values to Collect:**
```
aws_vpc_id       = "vpc-xxxxxxxxx"
aws_subnet_id    = "subnet-xxxxxxxxx"
aws_ami_id       = "ami-xxxxxxxxx"
aws_ssh_key_name = "univrs-nexus"
```

---

### 4. Azure Account Setup

- [ ] **Create Azure Account**
  - Go to: https://azure.microsoft.com/free/
  - Sign up for Free Tier ($200 credit + 12 months free services)

- [ ] **Install and Login Azure CLI**
  ```bash
  az login
  az account list --output table
  az account set --subscription "<subscription-id>"
  ```

- [ ] **Create Resource Group**
  ```bash
  az group create --name univrs-production-rg --location eastus
  ```

**Azure Values to Collect:**
```
azure_resource_group_name = "univrs-production-rg"
azure_location            = "eastus"
azure_ssh_public_key      = "ssh-ed25519 AAAA..."
```

---

### 5. GCP Account Setup

- [ ] **Create GCP Account**
  - Go to: https://cloud.google.com/free
  - Sign up (includes $300 credit + Always Free tier)

- [ ] **Create Project**
  - Navigate: Console → Select Project → New Project
  - Name: `univrs-production`
  - Note Project ID (may differ from name)

- [ ] **Enable Compute Engine API**
  ```bash
  gcloud services enable compute.googleapis.com
  ```

- [ ] **Configure gcloud CLI**
  ```bash
  gcloud auth login
  gcloud auth application-default login
  gcloud config set project <your-project-id>

  # Verify
  gcloud config list
  ```

**GCP Values to Collect:**
```
gcp_project        = "univrs-production"
gcp_region         = "us-central1"
gcp_zone           = "us-central1-a"
gcp_ssh_public_key = "ssh-ed25519 AAAA..."
```

---

## Deployment

### 6. Configure Terraform

```bash
cd ~/repos/univrs-orchestration/terraform/environments/production

# Copy example config
cp terraform.tfvars.example terraform.tfvars

# Edit with your values
vim terraform.tfvars
```

### 7. Deploy Cluster

```bash
# Initialize Terraform
terraform init

# Validate configuration
terraform validate

# Preview changes
terraform plan

# Deploy Bootstrap first (Oracle)
terraform apply -target=module.oracle_bootstrap

# Wait 60 seconds for bootstrap to initialize
sleep 60

# Deploy workers
terraform apply

# Verify
terraform output
```

### 8. Verify Cluster Health

```bash
# Get bootstrap IP
BOOTSTRAP_IP=$(terraform output -raw oracle_bootstrap_ip)

# Check health
curl http://$BOOTSTRAP_IP:9090/health

# Check cluster membership
curl http://$BOOTSTRAP_IP:9090/cluster/members

# SSH to bootstrap
ssh -i ~/.ssh/univrs-nexus opc@$BOOTSTRAP_IP

# View logs
ssh opc@$BOOTSTRAP_IP "docker logs univrs-orchestrator"
```

---

## Port Reference

| Port | Protocol | Service | Description |
|------|----------|---------|-------------|
| 22 | TCP | SSH | Admin access (restrict to your IP) |
| 7280 | UDP | Chitchat | Orchestrator gossip protocol |
| 8080 | TCP | HTTP/WS | Network REST API & WebSocket |
| 9000-9001 | TCP/UDP | libp2p | P2P transport (TCP & QUIC) |
| 9090 | TCP | HTTP | Orchestrator REST API |

---

## Troubleshooting

### Node Not Joining Cluster
```bash
# Check bootstrap health
curl http://<bootstrap-ip>:9090/health

# Check firewall allows UDP 7280
# Check worker can reach bootstrap
ssh <worker> "curl http://<bootstrap-ip>:7280"
```

### Container Not Starting
```bash
# Check Docker status
ssh <node> "systemctl status docker"

# Check cloud-init logs
ssh <node> "cat /var/log/cloud-init-output.log"

# Check container logs
ssh <node> "docker logs univrs-orchestrator"
```

---

## Teardown

```bash
# Destroy all resources
terraform destroy

# Or destroy specific cloud
terraform destroy -target=module.azure_worker
```

---

## Next Steps for New Nexus Operators

1. Complete this checklist to stand up your node
2. Share your node's public IP with the network coordinator
3. Your node will automatically join the gossip cluster
4. Monitor health at `http://<your-ip>:9090/health`

---

*Last updated: January 2026*
