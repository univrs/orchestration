cd ~/repos/RustOrchestration/ai_native_orchestrator && \
echo "=== Workspace members ===" && \
grep -A 20 "\[workspace\]" Cargo.toml && \
echo "" && \
echo "=== scheduler_interface contents ===" && \
ls -la scheduler_interface/src/ 2>/dev/null || echo "NOT FOUND" && \
echo "" && \
echo "=== orchestrator_core contents ===" && \
ls -la orchestrator_core/src/ 2>/dev/null || echo "NOT FOUND"
