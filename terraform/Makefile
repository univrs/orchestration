# ==============================================================================
# Univrs.io Multi-Cloud Terraform Makefile
# ==============================================================================

SHELL := /bin/bash
.PHONY: help init plan apply destroy validate format lint \
        apply-oracle apply-azure apply-aws apply-gcp \
        destroy-oracle destroy-azure destroy-aws destroy-gcp \
        health-check cluster-status output clean

# Default environment
ENV ?= production
TF_DIR := environments/$(ENV)

# Colors for output
RED := \033[0;31m
GREEN := \033[0;32m
YELLOW := \033[0;33m
BLUE := \033[0;34m
NC := \033[0m

# ==============================================================================
# Help
# ==============================================================================

help: ## Show this help
	@echo ""
	@echo "$(BLUE)Univrs.io Multi-Cloud Terraform$(NC)"
	@echo "================================="
	@echo ""
	@echo "$(YELLOW)Usage:$(NC)"
	@echo "  make <target> [ENV=production|staging]"
	@echo ""
	@echo "$(YELLOW)Targets:$(NC)"
	@awk 'BEGIN {FS = ":.*##"; } /^[a-zA-Z_-]+:.*?##/ { printf "  $(GREEN)%-20s$(NC) %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@echo ""
	@echo "$(YELLOW)Deployment Order:$(NC)"
	@echo "  1. make apply-oracle   (Bootstrap node first)"
	@echo "  2. make apply-workers  (Workers in parallel)"
	@echo "  3. make health-check   (Verify cluster)"
	@echo ""

# ==============================================================================
# Core Terraform Commands
# ==============================================================================

init: ## Initialize Terraform
	@echo "$(BLUE)Initializing Terraform for $(ENV)...$(NC)"
	cd $(TF_DIR) && terraform init

validate: ## Validate Terraform configuration
	@echo "$(BLUE)Validating Terraform configuration...$(NC)"
	cd $(TF_DIR) && terraform validate

format: ## Format Terraform files
	@echo "$(BLUE)Formatting Terraform files...$(NC)"
	terraform fmt -recursive

lint: validate ## Lint Terraform files
	@echo "$(BLUE)Linting Terraform files...$(NC)"
	@if command -v tflint &> /dev/null; then \
		tflint --recursive; \
	else \
		echo "$(YELLOW)tflint not installed, skipping...$(NC)"; \
	fi

plan: init validate ## Plan all infrastructure changes
	@echo "$(BLUE)Planning infrastructure for $(ENV)...$(NC)"
	cd $(TF_DIR) && terraform plan -out=tfplan

apply: init ## Apply all infrastructure changes
	@echo "$(BLUE)Applying infrastructure for $(ENV)...$(NC)"
	cd $(TF_DIR) && terraform apply -auto-approve

destroy: ## Destroy all infrastructure (DANGEROUS)
	@echo "$(RED)WARNING: This will destroy ALL infrastructure in $(ENV)!$(NC)"
	@read -p "Are you sure? Type 'yes' to confirm: " confirm; \
	if [ "$$confirm" = "yes" ]; then \
		cd $(TF_DIR) && terraform destroy -auto-approve; \
	else \
		echo "Cancelled."; \
	fi

# ==============================================================================
# Per-Cloud Deployment
# ==============================================================================

apply-oracle: init ## Deploy Oracle bootstrap node first
	@echo "$(BLUE)Deploying Oracle bootstrap node...$(NC)"
	cd $(TF_DIR) && terraform apply -auto-approve \
		-target=module.oracle_bootstrap
	@echo "$(GREEN)Oracle bootstrap deployed!$(NC)"
	@echo "$(YELLOW)Waiting 60s for bootstrap to initialize...$(NC)"
	@sleep 60

apply-azure: ## Deploy Azure worker node
	@echo "$(BLUE)Deploying Azure worker node...$(NC)"
	cd $(TF_DIR) && terraform apply -auto-approve \
		-target=module.azure_worker

apply-aws: ## Deploy AWS worker node
	@echo "$(BLUE)Deploying AWS worker node...$(NC)"
	cd $(TF_DIR) && terraform apply -auto-approve \
		-target=module.aws_worker

apply-gcp: ## Deploy GCP worker node
	@echo "$(BLUE)Deploying GCP worker node...$(NC)"
	cd $(TF_DIR) && terraform apply -auto-approve \
		-target=module.gcp_worker

apply-workers: apply-azure apply-aws apply-gcp ## Deploy all worker nodes
	@echo "$(GREEN)All workers deployed!$(NC)"

# ==============================================================================
# Per-Cloud Destruction
# ==============================================================================

destroy-oracle: ## Destroy Oracle bootstrap node (DANGEROUS)
	@echo "$(RED)Destroying Oracle bootstrap node...$(NC)"
	cd $(TF_DIR) && terraform destroy -auto-approve \
		-target=module.oracle_bootstrap

destroy-azure: ## Destroy Azure worker node
	@echo "$(RED)Destroying Azure worker node...$(NC)"
	cd $(TF_DIR) && terraform destroy -auto-approve \
		-target=module.azure_worker

destroy-aws: ## Destroy AWS worker node
	@echo "$(RED)Destroying AWS worker node...$(NC)"
	cd $(TF_DIR) && terraform destroy -auto-approve \
		-target=module.aws_worker

destroy-gcp: ## Destroy GCP worker node
	@echo "$(RED)Destroying GCP worker node...$(NC)"
	cd $(TF_DIR) && terraform destroy -auto-approve \
		-target=module.gcp_worker

destroy-workers: destroy-azure destroy-aws destroy-gcp ## Destroy all workers
	@echo "$(YELLOW)All workers destroyed.$(NC)"

# ==============================================================================
# Health & Status
# ==============================================================================

health-check: ## Check health of all nodes
	@echo "$(BLUE)Checking cluster health...$(NC)"
	@echo ""
	@BOOTSTRAP_IP=$$(cd $(TF_DIR) && terraform output -raw bootstrap_node 2>/dev/null | jq -r '.public_ip' 2>/dev/null || echo ""); \
	if [ -n "$$BOOTSTRAP_IP" ] && [ "$$BOOTSTRAP_IP" != "null" ]; then \
		echo "$(YELLOW)Bootstrap (Oracle):$(NC)"; \
		curl -sf "http://$$BOOTSTRAP_IP:9090/health" && echo " $(GREEN)✓ Orchestrator OK$(NC)" || echo " $(RED)✗ Orchestrator FAILED$(NC)"; \
		curl -sf "http://$$BOOTSTRAP_IP:8080/health" && echo " $(GREEN)✓ Network OK$(NC)" || echo " $(RED)✗ Network FAILED$(NC)"; \
	fi
	@echo ""
	@for cloud in azure aws gcp; do \
		IP=$$(cd $(TF_DIR) && terraform output -json worker_nodes 2>/dev/null | jq -r ".$$cloud.public_ip // empty" 2>/dev/null); \
		if [ -n "$$IP" ] && [ "$$IP" != "null" ]; then \
			echo "$(YELLOW)Worker ($$cloud):$(NC)"; \
			curl -sf "http://$$IP:9090/health" && echo " $(GREEN)✓ Orchestrator OK$(NC)" || echo " $(RED)✗ Orchestrator FAILED$(NC)"; \
			curl -sf "http://$$IP:8080/health" && echo " $(GREEN)✓ Network OK$(NC)" || echo " $(RED)✗ Network FAILED$(NC)"; \
			echo ""; \
		fi; \
	done

cluster-status: ## Show cluster membership
	@echo "$(BLUE)Cluster membership status...$(NC)"
	@BOOTSTRAP_IP=$$(cd $(TF_DIR) && terraform output -raw bootstrap_node 2>/dev/null | jq -r '.public_ip' 2>/dev/null || echo ""); \
	if [ -n "$$BOOTSTRAP_IP" ] && [ "$$BOOTSTRAP_IP" != "null" ]; then \
		echo "Querying $$BOOTSTRAP_IP:9090..."; \
		curl -sf "http://$$BOOTSTRAP_IP:9090/api/v1/cluster/nodes" | jq; \
	else \
		echo "$(RED)Bootstrap IP not found. Is the cluster deployed?$(NC)"; \
	fi

output: ## Show all Terraform outputs
	@cd $(TF_DIR) && terraform output

output-json: ## Show all Terraform outputs as JSON
	@cd $(TF_DIR) && terraform output -json

# ==============================================================================
# Utilities
# ==============================================================================

clean: ## Clean Terraform cache and plan files
	@echo "$(BLUE)Cleaning Terraform cache...$(NC)"
	find . -type d -name ".terraform" -exec rm -rf {} + 2>/dev/null || true
	find . -name "tfplan" -delete 2>/dev/null || true
	find . -name ".terraform.lock.hcl" -delete 2>/dev/null || true
	@echo "$(GREEN)Cleaned!$(NC)"

ssh-oracle: ## SSH to Oracle bootstrap node
	@BOOTSTRAP_IP=$$(cd $(TF_DIR) && terraform output -raw bootstrap_node 2>/dev/null | jq -r '.public_ip' 2>/dev/null); \
	if [ -n "$$BOOTSTRAP_IP" ] && [ "$$BOOTSTRAP_IP" != "null" ]; then \
		ssh ubuntu@$$BOOTSTRAP_IP; \
	else \
		echo "$(RED)Bootstrap IP not found.$(NC)"; \
	fi

logs-oracle: ## Show container logs from Oracle bootstrap
	@BOOTSTRAP_IP=$$(cd $(TF_DIR) && terraform output -raw bootstrap_node 2>/dev/null | jq -r '.public_ip' 2>/dev/null); \
	if [ -n "$$BOOTSTRAP_IP" ] && [ "$$BOOTSTRAP_IP" != "null" ]; then \
		ssh ubuntu@$$BOOTSTRAP_IP "docker logs univrs-orchestrator --tail 100"; \
	fi

# ==============================================================================
# Full Deployment Workflow
# ==============================================================================

deploy-all: apply-oracle apply-workers health-check ## Deploy entire cluster
	@echo ""
	@echo "$(GREEN)==========================================$(NC)"
	@echo "$(GREEN)Univrs.io cluster deployed successfully!$(NC)"
	@echo "$(GREEN)==========================================$(NC)"
	@echo ""
	@$(MAKE) output

# Default target
.DEFAULT_GOAL := help
