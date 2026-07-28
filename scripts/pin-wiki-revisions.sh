#!/usr/bin/env bash
# scripts/pin-wiki-revisions.sh — pin current MediaWiki revision IDs onto wiki_pages
set -o errexit -o pipefail
export TZ='Asia/Jakarta'
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
source .env

psql "$DATABASE_URL" -Atc "SELECT title FROM wiki_pages WHERE wiki_lang='en' ORDER BY title;" | while IFS= read -r title; do
  [ -z "$title" ] && continue
  enc=$(python3 -c "import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1]))" "$title")
  json=$(curl -fsSL -A "TalariaV2/0.1 (research; revision pin)" \
    "https://en.wikipedia.org/w/api.php?action=query&titles=${enc}&redirects=1&prop=revisions&rvprop=ids&format=json")
  revid=$(python3 -c "import json,sys; d=json.load(sys.stdin); pages=d.get('query',{}).get('pages',{}); print(next(iter(pages.values())).get('revisions',[{}])[0].get('revid',''))" <<<"$json")
  echo "$title -> $revid"
  if [ -n "$revid" ]; then
    psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c \
      "UPDATE wiki_pages SET revision_id = ${revid} WHERE title = '${title//\'/\'\'}' AND wiki_lang = 'en';"
  fi
done
