echo "=== Structure ===" && \
find . -name "*.rs" -type f | head -50 && \
echo "" && \
echo "=== Cargo.toml ===" && \
cat Cargo.toml && \
echo "" && \
echo "=== Modules ===" && \
find . -name "mod.rs" -exec echo "---" \; -exec head -20 {} \;
