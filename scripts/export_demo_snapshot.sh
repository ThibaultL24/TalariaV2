#!/usr/bin/env bash
# scripts/export_demo_snapshot.sh — freeze the current Postgres for a shareable demo.
set -euo pipefail
cd "$(dirname "$0")/.."
set -a
# shellcheck disable=SC1091
[[ -f .env ]] && . ./.env
set +a

: "${DATABASE_URL:?DATABASE_URL required}"

STAMP="${DEMO_SNAPSHOT_STAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUT_DIR="${DEMO_SNAPSHOT_DIR:-demo/snapshots}"
mkdir -p "$OUT_DIR"
DUMP="$OUT_DIR/talaria-demo-${STAMP}.dump"
MANIFEST="$OUT_DIR/talaria-demo-${STAMP}.manifest.json"
LATEST_DUMP="$OUT_DIR/talaria-demo-latest.dump"
LATEST_MANIFEST="$OUT_DIR/talaria-demo-latest.manifest.json"

echo "==> exporting $DATABASE_URL → $DUMP"
if command -v pg_dump >/dev/null 2>&1; then
  pg_dump --format=custom --compress=9 --no-owner --no-acl \
    --file="$DUMP" "$DATABASE_URL"
else
  # Fallback: dump from the compose PostGIS container.
  sudo docker compose exec -T db pg_dump -U postgres -d talaria_engine_development \
    --format=custom --compress=9 --no-owner --no-acl >"$DUMP"
fi

echo "==> writing manifest $MANIFEST"
python3 - <<PY
import json, os, subprocess

dump = "$DUMP"
manifest_path = "$MANIFEST"
stamp = "$STAMP"
url = os.environ["DATABASE_URL"]

roster_sql = """
SELECT e.qid, e.canonical_name,
  COUNT(*) FILTER (WHERE ce.is_active AND ce.pipeline = 'person'),
  COUNT(*) FILTER (WHERE ce.is_active AND ce.pipeline = 'person' AND ce.map_eligible AND ce.geom IS NOT NULL),
  (SELECT COUNT(*) FROM soft_claims sc WHERE sc.entity_id = e.id)
FROM entities e
LEFT JOIN canonical_events ce ON ce.entity_id = e.id
WHERE e.qid IN ('Q517','Q687','Q7186','Q535','Q7226','Q7742','Q3052772','Q22686','Q2042','Q76')
GROUP BY e.id, e.qid, e.canonical_name
ORDER BY 3 DESC;
"""
out = subprocess.check_output(["psql", url, "-At", "-F", "|", "-c", roster_sql], text=True)
roster = []
for line in out.splitlines():
    if not line.strip():
        continue
    qid, name, events, pins, claims = line.split("|")
    roster.append({
        "qid": qid,
        "name": name,
        "events": int(events),
        "pins": int(pins),
        "claims": int(claims),
    })

totals_raw = subprocess.check_output([
    "psql", url, "-At", "-F", "|", "-c",
    "SELECT (SELECT COUNT(*) FROM entities),"
    " (SELECT COUNT(*) FROM canonical_events WHERE is_active AND pipeline='person'),"
    " (SELECT COUNT(*) FROM soft_claims),"
    " (SELECT COUNT(*) FROM raw_documents);",
], text=True).strip().split("|")

manifest = {
    "stamp": stamp,
    "dump_file": os.path.basename(dump),
    "dump_bytes": os.path.getsize(dump),
    "created_at": stamp,
    "notes": (
        "Person-pipeline demo snapshot. Restore with scripts/restore_demo_snapshot.sh. "
        "Beta ingest works without OPENAI_API_KEY (structured Wikipedia/Wikidata + catalogs)."
    ),
    "totals": {
        "entities": int(totals_raw[0]),
        "person_events": int(totals_raw[1]),
        "claims": int(totals_raw[2]),
        "raw_documents": int(totals_raw[3]),
    },
    "roster": roster,
}
with open(manifest_path, "w", encoding="utf-8") as f:
    json.dump(manifest, f, ensure_ascii=False, indent=2)
    f.write("\n")
print(json.dumps(manifest, ensure_ascii=False, indent=2))
PY

cp -f "$DUMP" "$LATEST_DUMP"
cp -f "$MANIFEST" "$LATEST_MANIFEST"
ls -lh "$DUMP" "$LATEST_DUMP" "$MANIFEST"

if [[ "${DEMO_SNAPSHOT_UPLOAD:-}" == "1" ]] && [[ -n "${S3_BUCKET:-}" ]]; then
  echo "==> uploading snapshot to object storage"
  ./scripts/sync_object_storage.sh push "$LATEST_DUMP" || echo "upload failed (non-fatal)" >&2
  ./scripts/sync_object_storage.sh push "$LATEST_MANIFEST" || true
fi

echo "==> done. Restore: ./scripts/restore_demo_snapshot.sh"
