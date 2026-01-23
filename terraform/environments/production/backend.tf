# Terraform Backend Configuration
#
# This uses S3 for state storage with DynamoDB for locking.
# Uncomment and configure for production use.
#
# Prerequisites:
# 1. Create S3 bucket: aws s3 mb s3://univrs-terraform-state
# 2. Enable versioning: aws s3api put-bucket-versioning --bucket univrs-terraform-state --versioning-configuration Status=Enabled
# 3. Create DynamoDB table: aws dynamodb create-table --table-name univrs-terraform-locks --attribute-definitions AttributeName=LockID,AttributeType=S --key-schema AttributeName=LockID,KeyType=HASH --billing-mode PAY_PER_REQUEST

# terraform {
#   backend "s3" {
#     bucket         = "univrs-terraform-state"
#     key            = "production/terraform.tfstate"
#     region         = "us-east-1"
#     dynamodb_table = "univrs-terraform-locks"
#     encrypt        = true
#   }
# }

# For initial development, use local backend
terraform {
  backend "local" {
    path = "terraform.tfstate"
  }
}
