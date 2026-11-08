# scripts/poreia/build_pilot_seed.py
"""Check the Poreia pilot catalog. This script never opens a database."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CATALOG = ROOT / "fixtures" / "poreia" / "pilot_catalog.json"
SQL = ROOT / "scripts" / "poreia" / "pilot_stage.sql"
EXPECTED_QIDS = ("Q59210966", "Q64436", "Q315318", "Q17076145")


def load_catalog() -> dict:
    return json.loads(CATALOG.read_text(encoding="utf-8"))


def check(catalog: dict, sql: str) -> list[str]:
    errors: list[str] = []
    if catalog.get("publication_status") != "quarantine":
        errors.append("catalog publication_status must stay quarantine")
    sites = catalog.get("sites") or []
    qids = [site.get("wikidata_qid") for site in sites]
    if qids != list(EXPECTED_QIDS):
        errors.append(f"unexpected site order: {qids}")
    for site in sites:
        lat = site["coordinates"]["lat"]
        lon = site["coordinates"]["lon"]
        if not (-90 <= lat <= 90 and -180 <= lon <= 180):
            errors.append(f"coordinates out of range for {site['wikidata_qid']}")
        if not str(site["source_url"]).startswith("https://www.wikidata.org/wiki/"):
            errors.append(f"source must be a Wikidata page for {site['wikidata_qid']}")
        if site["wikidata_qid"] == "Q17076145" and site["relation_role"] != "thematic":
            errors.append("the Arles foundation must stay a thematic link")
        if site["wikidata_qid"] == "Q17076145" and site["relation_pid"] is not None:
            errors.append("the Arles foundation must not use a visit or residence PID")
        if site["wikidata_qid"] == "Q59210966" and site["relation_pid"] == "P119":
            errors.append("the Invalides tomb must not claim P119")
        if site["wikidata_qid"] == "Q315318" and site["relation_pid"] != "P138":
            errors.append("the Langlois bridge relation must stay P138")
        if site["wikidata_qid"] == "Q64436" and site["relation_pid"] != "P88":
            errors.append("the Arc de Triomphe relation must stay P88")
        if site["wikidata_qid"] not in sql:
            errors.append(f"sql is missing {site['wikidata_qid']}")
    for occurrence in catalog.get("occurrences") or []:
        if occurrence.get("time_precision") != "day":
            errors.append("pilot occurrences must use day precision")
        for field in ("occurs_on", "ends_on"):
            value = occurrence[field]
            if "T" in value or ":" in value:
                errors.append(f"{field} must not contain a clock time")
        if occurrence["wikidata_qid"] not in sql:
            errors.append("sql is missing the exhibition")
    if "canonical_events" in sql:
        errors.append("pilot sql must not mention canonical_events")
    if "publication_status = 'published'" in sql or 'publication_status = "published"' in sql:
        errors.append("pilot sql must not publish rows")
    if sql.count("ON CONFLICT") < 4:
        errors.append("pilot sql must be idempotent for sites, narratives, media and occurrences")
    if "'quarantine'" not in sql:
        errors.append("pilot sql must insert quarantine rows")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        print("pass --check; this command does not write a database", file=sys.stderr)
        return 2
    errors = check(load_catalog(), SQL.read_text(encoding="utf-8"))
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("pilot catalog checked: 4 quarantine sites, no publication")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
