# scripts/poreia/merge_openapi_contract.py
"""Insert Poreia paths into the canonical OpenAPI file without replacing it."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CANONICAL = ROOT / "docs" / "openapi-v3.yml"
OVERLAY = ROOT / "docs" / "poreia-openapi-overlay.yml"
MARKER = "  /api/v1/entities/{entity_id}/overview:"


def merge(canonical: str, overlay: str) -> str:
    if "openapi:" in overlay.splitlines()[0]:
        raise SystemExit("overlay must be a path fragment, not a full contract")
    missing = [
        path
        for path in (
            "  /api/v1/poreia/sites:",
            "  /api/v1/poreia/occurrences:",
            "  /api/v1/admin/poreia/sites/{site_id}/review:",
        )
        if path not in overlay
    ]
    if missing:
        raise SystemExit(f"overlay is missing {missing}")
    if MARKER not in canonical:
        raise SystemExit("canonical contract no longer has the expected entity path")
    fresh = overlay.strip("\n")
    if "  /api/v1/poreia/sites:" in canonical:
        start = canonical.index("  /api/v1/poreia/sites:")
        end = canonical.index(MARKER)
        return canonical[:start] + fresh + "\n" + canonical[end:]
    return canonical.replace(MARKER, fresh + "\n" + MARKER, 1)


def main() -> int:
    merged = merge(CANONICAL.read_text(encoding="utf-8"), OVERLAY.read_text(encoding="utf-8"))
    if MARKER not in merged or "  /api/v1/admin/ingestions/summary:" not in merged:
        raise SystemExit("merge would drop existing paths")
    CANONICAL.write_text(merged, encoding="utf-8")
    print(f"merged Poreia paths into {CANONICAL}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
