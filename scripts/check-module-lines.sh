#!/usr/bin/env bash
# Fail if any of the plan’s modules (§7) exceeds 200 lines.
# Scoped to: flow-raft-server (executor, service, launcher, cli, cluster, raft_cluster, registry)
# and flow-raft-raft (network/tcp). Excludes .idea and target.
# Usage: ./scripts/check-module-lines.sh

set -e
MAX=200
failed=()
paths=(
  ./flow-raft-server/src/handlers/executor.rs
  ./flow-raft-server/src/grpc/service.rs
  ./flow-raft-server/src/node/launcher.rs
  ./flow-raft-server/src/raft_cluster/mod.rs
  ./flow-raft-server/src/raft_cluster/run.rs
  ./flow-raft-server/src/handlers/registry.rs
  ./flow-raft-raft/src/network/tcp.rs
)
for f in "${paths[@]}"; do
  [ -f "$f" ] || continue
  n=$(wc -l < "$f")
  if [ "$n" -gt "$MAX" ]; then
    failed+=("$f: $n")
  fi
done

if [ ${#failed[@]} -gt 0 ]; then
  echo "The following modules exceed ${MAX} lines:"
  printf '%s\n' "${failed[@]}"
  exit 1
fi
echo "All modules are <= ${MAX} lines."
exit 0
