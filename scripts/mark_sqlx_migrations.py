#!/usr/bin/env python3
# scripts/mark_sqlx_migrations.py — emit SQL to mark all migrations applied (checksum-accurate).
"""Print SQL that TRUNCATEs and re-fills _sqlx_migrations with correct sqlx SHA-384 checksums."""

from __future__ import annotations

import hashlib
import pathlib
import re
import sys


def main() -> int:
    root = pathlib.Path(__file__).resolve().parents[1]
    mig_dir = root / "migrations"
    rows: list[tuple[int, str, bytes]] = []
    for path in sorted(mig_dir.glob("*.sql")):
        match = re.match(r"^(\d+)_(.+)\.sql$", path.name)
        if not match:
            continue
        version = int(match.group(1))
        description = match.group(2).replace("_", " ")
        digest = hashlib.sha384(path.read_bytes()).digest()
        rows.append((version, description, digest))

    print("BEGIN;")
    print("TRUNCATE _sqlx_migrations;")
    for version, description, digest in rows:
        desc_esc = description.replace("'", "''")
        print(
            "INSERT INTO _sqlx_migrations "
            "(version, description, installed_on, success, checksum, execution_time) "
            f"VALUES ({version}, '{desc_esc}', NOW(), true, "
            f"decode('{digest.hex()}', 'hex'), 0);"
        )
    print("COMMIT;")
    print(f"-- marked {len(rows)} migrations", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
