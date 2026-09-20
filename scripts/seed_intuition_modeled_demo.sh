#!/usr/bin/env bash
# scripts/seed_intuition_modeled_demo.sh
# Seed intuition_publications (pending) via MetaSudo model sidecar.
# Use when `talaria intuition-plan` cannot reach Postgres from the agent sandbox.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SUBJECT_QID="${1:-Q517}"
LIMIT="${2:-12}"
SIDECAR="$ROOT/sidecar/intuition"
export PGPASSWORD="${PGPASSWORD:-postgres}"
PSQL=(psql -h 127.0.0.1 -p 5433 -U postgres -d talaria_engine_development -v ON_ERROR_STOP=1)

slug() {
  python3 -c 'import sys,unicodedata
raw=sys.argv[1]
ascii=unicodedata.normalize("NFKD", raw).encode("ascii","ignore").decode().lower()
out=[]; prev=False
for ch in ascii:
  if ch.isalnum(): out.append(ch); prev=False
  elif not prev: out.append("-"); prev=True
print("".join(out).strip("-") or "x")' "$1"
}

ENTITY_ID="$("${PSQL[@]}" -tAc "SELECT id::text FROM entities WHERE qid='${SUBJECT_QID}' ORDER BY created_at ASC LIMIT 1;")"
[[ -n "$ENTITY_ID" ]] || { echo "No entity for qid=$SUBJECT_QID" >&2; exit 1; }
LABEL="$("${PSQL[@]}" -tAc "SELECT COALESCE(canonical_name, wikipedia_title) FROM entities WHERE id='${ENTITY_ID}';")"
LABEL="$(echo "$LABEL" | xargs)"

echo "Seeding MetaSudo modeled theories for ${LABEL} (${ENTITY_ID}), limit=${LIMIT}"

TMP="$(mktemp)"
"${PSQL[@]}" -tAc "
SELECT id::text || E'\t' || replace(replace(text, E'\t', ' '), E'\n', ' ')
FROM soft_claims
WHERE entity_id='${ENTITY_ID}' AND claim_kind='theory'
ORDER BY created_at ASC
LIMIT ${LIMIT};
" >"$TMP"

COUNT=0
while IFS=$'\t' read -r CLAIM_ID TEXT; do
  [[ -z "${CLAIM_ID:-}" ]] && continue
  Q_FRAG="$(slug "about-${LABEL}")"
  P_FRAG="$(slug "claim-${CLAIM_ID}")"
  DEBATE_ID="talaria:debate:${Q_FRAG}:${P_FRAG}"

  FACT_JSON="$(CLAIM_ID="$CLAIM_ID" DEBATE_ID="$DEBATE_ID" LABEL="$LABEL" TEXT="$TEXT" python3 - <<'PY'
import json, os
print(json.dumps({
  "version": "talaria.intuition_canon.v2",
  "debate_id": os.environ["DEBATE_ID"],
  "kind": "theory",
  "question": {"text": f"What is claimed about {os.environ['LABEL']}?"},
  "proposition": {"text": os.environ["TEXT"]},
  "about_event": None,
}))
PY
)"

  OUT="$(cd "$SIDECAR" && ./node_modules/.bin/tsx cli.ts model "$FACT_JSON")"
  GRAPH="$(echo "$OUT" | python3 -c 'import json,sys; print(json.dumps(json.load(sys.stdin)["graph"]))')"
  FP="$(echo "$FACT_JSON" | python3 -c 'import json,sys,hashlib
f=json.load(sys.stdin)
payload={"version":f["version"],"kind":f["kind"],"question":f["question"]["text"],"proposition":f["proposition"]["text"],"canonical_event_id":None}
print(hashlib.sha256(json.dumps(payload,separators=(",",":"),ensure_ascii=False).encode()).hexdigest())')"

  PAYLOAD="$(FACT_JSON="$FACT_JSON" GRAPH="$GRAPH" python3 - <<'PY'
import json, os
print(json.dumps({
  "version": "talaria.intuition_canon.v2",
  "fact": json.loads(os.environ["FACT_JSON"]),
  "graph": json.loads(os.environ["GRAPH"]),
}))
PY
)"

  DEBATE_SQL="$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$DEBATE_ID")"
  FP_SQL="$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$FP")"

  "${PSQL[@]}" <<SQL >/dev/null
INSERT INTO intuition_publications (
  subject_entity_id, debate_id, bundle_fingerprint, kind, status, payload_json
) VALUES (
  '${ENTITY_ID}'::uuid,
  ${DEBATE_SQL},
  ${FP_SQL},
  'theory',
  'pending',
  \$json\$${PAYLOAD}\$json\$::jsonb
)
ON CONFLICT (bundle_fingerprint) DO UPDATE SET
  payload_json = EXCLUDED.payload_json,
  status = CASE
    WHEN intuition_publications.status = 'published' THEN intuition_publications.status
    ELSE EXCLUDED.status
  END,
  debate_id = EXCLUDED.debate_id,
  updated_at = NOW();
SQL

  COUNT=$((COUNT + 1))
  echo "  modeled ${COUNT}: ${CLAIM_ID:0:8}…"
done <"$TMP"
rm -f "$TMP"

"${PSQL[@]}" -c "SELECT status, kind, COUNT(*) FROM intuition_publications GROUP BY 1,2 ORDER BY 3 DESC;"
echo "Done: ${COUNT} MetaSudo-modeled theories pending publish."
