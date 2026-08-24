#!/usr/bin/env bash
# Automates the manual flow from README.md:
#   1. build perp + liquidity-pool
#   2. surfpool start (auto-deploys perp) in the background, unsupervised
#   3. surfpool run deployment --env localnet for liquidity-pool, unsupervised
#   4. anchor test against that running surfnet
# surfpool is always killed on exit, success or failure.
set -euo pipefail

PERP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LP_DIR="$(cd "$PERP_DIR/../liquidity-pool" && pwd)"
RPC_URL="http://127.0.0.1:8899"
PERP_PROGRAM_ID="$(sed -n 's/^perp = "\(.*\)"/\1/p' "$PERP_DIR/Anchor.toml")"
LP_PROGRAM_ID="$(sed -n 's/^liquidity_pool = "\(.*\)"/\1/p' "$LP_DIR/Anchor.toml")"

SURFPOOL_PID=""

cleanup() {
  if [[ -n "$SURFPOOL_PID" ]] && kill -0 "$SURFPOOL_PID" 2>/dev/null; then
    echo "==> Stopping surfpool (pid $SURFPOOL_PID)"
    kill "$SURFPOOL_PID" 2>/dev/null || true
    wait "$SURFPOOL_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

wait_for_rpc() {
  echo "==> Waiting for surfnet RPC at $RPC_URL"
  for _ in $(seq 1 60); do
    if curl -s -o /dev/null -X POST -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' "$RPC_URL"; then
      return 0
    fi
    sleep 1
  done
  echo "Timed out waiting for surfnet RPC" >&2
  exit 1
}

wait_for_program() {
  local program_id="$1"
  echo "==> Waiting for program $program_id to be deployed"
  for _ in $(seq 1 120); do
    if solana program show "$program_id" --url "$RPC_URL" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "Timed out waiting for program $program_id to deploy" >&2
  exit 1
}

echo "==> Building perp"
(cd "$PERP_DIR" && anchor build)

echo "==> Building liquidity-pool"
(cd "$LP_DIR" && anchor build)

mkdir -p "$PERP_DIR/.surfpool/logs"
echo "==> Starting surfnet in $PERP_DIR (auto-deploys perp)"
(cd "$PERP_DIR" && exec surfpool start --no-tui --no-studio -y) \
  >"$PERP_DIR/.surfpool/logs/start.log" 2>&1 &
SURFPOOL_PID=$!

wait_for_rpc
wait_for_program "$PERP_PROGRAM_ID"

echo "==> Deploying liquidity-pool into the running surfnet"
(cd "$LP_DIR" && surfpool run deployment --env localnet --unsupervised)
wait_for_program "$LP_PROGRAM_ID"

echo "==> Running integration tests"
(cd "$PERP_DIR" && anchor test)
