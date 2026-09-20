#!/usr/bin/env bash
# scripts/sync_object_storage.sh — S3-compatible upload/download (Cloudflare R2, Backblaze B2, MinIO…).
# Requires: aws CLI v2
#
# Env (all required for push/pull unless noted):
#   S3_ENDPOINT     e.g. https://<accountid>.r2.cloudflarestorage.com
#                   or   https://s3.us-west-004.backblazeb2.com
#   S3_BUCKET       bucket name
#   S3_ACCESS_KEY / S3_SECRET_KEY
#   S3_REGION       default auto (R2) or us-west-004 (B2)
#   S3_PREFIX       optional key prefix, default talaria/
#
# Usage:
#   ./scripts/sync_object_storage.sh push demo/snapshots/talaria-demo-latest.dump
#   ./scripts/sync_object_storage.sh pull demo/snapshots/talaria-demo-latest.dump
#   ./scripts/sync_object_storage.sh push-dir "$TALARIA_DATA_ROOT/dumps"
set -euo pipefail
cd "$(dirname "$0")/.."

CMD="${1:-}"
SRC="${2:-}"

: "${S3_ENDPOINT:?S3_ENDPOINT required}"
: "${S3_BUCKET:?S3_BUCKET required}"
: "${S3_ACCESS_KEY:?S3_ACCESS_KEY required}"
: "${S3_SECRET_KEY:?S3_SECRET_KEY required}"
REGION="${S3_REGION:-auto}"
PREFIX="${S3_PREFIX:-talaria/}"
PREFIX="${PREFIX%/}/"

if ! command -v aws >/dev/null 2>&1; then
  echo "aws CLI not found (needed for S3-compatible sync)." >&2
  exit 1
fi

export AWS_ACCESS_KEY_ID="$S3_ACCESS_KEY"
export AWS_SECRET_ACCESS_KEY="$S3_SECRET_KEY"
export AWS_DEFAULT_REGION="$REGION"

aws_s3() {
  aws --endpoint-url="$S3_ENDPOINT" s3 "$@"
}

case "$CMD" in
  push)
    [[ -n "$SRC" && -f "$SRC" ]] || { echo "usage: $0 push <file>"; exit 2; }
    KEY="${PREFIX}$(basename "$SRC")"
    echo "==> upload $SRC → s3://${S3_BUCKET}/${KEY}"
    aws_s3 cp "$SRC" "s3://${S3_BUCKET}/${KEY}"
    ;;
  pull)
    DEST="${SRC:-}"
    [[ -n "$DEST" ]] || { echo "usage: $0 pull <local-path>"; exit 2; }
    KEY="${PREFIX}$(basename "$DEST")"
    mkdir -p "$(dirname "$DEST")"
    echo "==> download s3://${S3_BUCKET}/${KEY} → $DEST"
    aws_s3 cp "s3://${S3_BUCKET}/${KEY}" "$DEST"
    ;;
  push-dir)
    [[ -n "$SRC" && -d "$SRC" ]] || { echo "usage: $0 push-dir <dir>"; exit 2; }
    NAME="$(basename "$SRC")"
    echo "==> sync $SRC → s3://${S3_BUCKET}/${PREFIX}${NAME}/"
    aws_s3 sync "$SRC" "s3://${S3_BUCKET}/${PREFIX}${NAME}/"
    ;;
  *)
    echo "usage: $0 push|pull|push-dir <path>" >&2
    exit 2
    ;;
esac
echo "==> done"
