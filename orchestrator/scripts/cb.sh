cargo build --workspace 2>&1 | grep -E "(warning|error)" || echo "Clean build! ✅"
