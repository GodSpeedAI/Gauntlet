#!/usr/bin/env bash
# scripts/load-env.sh — source (do not execute) to load the dev env layers:
#   1. .env.defaults  committed, non-secret; `${VAR:-default}` so your shell wins
#   2. .env           gitignored, personal overrides and secrets; wins over both
# See .env.defaults for why this is separate from the pinned .env.example.
DEV_REPO_ROOT="${DEV_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
export DEV_REPO_ROOT
set -a
# shellcheck source=/dev/null
[ -f "$DEV_REPO_ROOT/.env.defaults" ] && . "$DEV_REPO_ROOT/.env.defaults"
# shellcheck source=/dev/null
[ -f "$DEV_REPO_ROOT/.env" ] && . "$DEV_REPO_ROOT/.env"
set +a
