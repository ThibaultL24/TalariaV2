#!/usr/bin/env python3
# scripts/seed_demo_dump.py — multi-profile dump with fixture anecdotes + optional live extracts.
"""Pack biography pages into a synthetic Wikipedia multistream dump."""

from __future__ import annotations

import bz2
import json
import os
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures" / "dumps"
DATA_ROOT = Path(os.environ.get("TALARIA_DATA_ROOT", "/home/ubuntu/wiki-dump"))
DUMPS = DATA_ROOT / "dumps"
DUMPS.mkdir(parents=True, exist_ok=True)

UA = {"User-Agent": "TalariaEngine/0.1 (demo-dump-seed; research)"}
FETCH = os.environ.get("TALARIA_FETCH_WIKI", "1") not in {"0", "false", "no"}

BIO_TITLES = [
    "Napoleon",
    "Marie Curie",
    "Victor Hugo",
    "Leonardo da Vinci",
    "Christopher Columbus",
    "Alan Turing",
    "Cleopatra",
]

EXTRA_TITLES = [
    "Military career of Napoleon",
    "Battle of Austerlitz",
    "Battle of Waterloo",
    "Battle of Borodino",
    "Radium",
    "Pierre Curie",
    "Les Misérables",
    "Notre-Dame de Paris",
    "The Last Supper (Leonardo da Vinci)",
    "Mona Lisa",
    "Voyages of Christopher Columbus",
    "Bletchley Park",
    "Enigma machine",
    "Ptolemaic Kingdom",
    "Battle of Actium",
]


def fetch_extract(title: str) -> tuple[str, str, int] | None:
    url = "https://en.wikipedia.org/w/api.php?" + urllib.parse.urlencode(
        {
            "action": "query",
            "prop": "extracts|info",
            "explaintext": 1,
            "titles": title,
            "format": "json",
            "redirects": 1,
            "inprop": "url",
            "exlimit": 1,
        }
    )
    req = urllib.request.Request(url, headers=UA)
    try:
        with urllib.request.urlopen(req, timeout=45) as resp:
            data = json.load(resp)
    except Exception as exc:  # noqa: BLE001 — seed must succeed offline
        print(f"skip fetch {title}: {exc}")
        return None
    page = next(iter(data.get("query", {}).get("pages", {}).values()), None)
    if not page or page.get("extract") is None:
        return None
    return page.get("title", title), page["extract"], int(page.get("pageid") or 0)


def xml_escape(text: str) -> str:
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def page_xml(page_id: int, rev_id: int, title: str, text: str) -> str:
    return f"""  <page>
    <title>{xml_escape(title)}</title>
    <ns>0</ns>
    <id>{page_id}</id>
    <revision>
      <id>{rev_id}</id>
      <text>{xml_escape(text)}</text>
    </revision>
  </page>"""


def load_fixtures() -> dict[str, str]:
    pages: dict[str, str] = {}
    if not FIXTURES.exists():
        return pages
    for path in sorted(FIXTURES.glob("*.txt")):
        title = path.stem
        pages[title] = path.read_text(encoding="utf-8").strip() + "\n"
    return pages


def main() -> None:
    fixtures = load_fixtures()
    pages: list[tuple[int, int, str, str]] = []
    seen: set[str] = set()

    titles = BIO_TITLES + EXTRA_TITLES
    for i, title in enumerate(titles):
        extract = ""
        resolved = title
        page_id = 3000 + i
        if FETCH:
            got = fetch_extract(title)
            if got:
                resolved, extract, fetched_id = got
                page_id = fetched_id or page_id
                print(f"ok fetch {resolved!r}: {len(extract)} chars")
            else:
                print(f"no fetch: {title}")
        fixture = fixtures.get(title, "")
        if fixture:
            extract = (extract + "\n\n" + fixture).strip() if extract else fixture
        if not extract:
            continue
        if resolved in seen:
            print(f"skip dup: {resolved}")
            continue
        seen.add(resolved)
        pages.append((page_id, 9000 + i, resolved, extract))

    for title, fixture in fixtures.items():
        if title in seen:
            continue
        pages.append((8000 + len(pages), 9100 + len(pages), title, fixture))
        seen.add(title)
        print(f"ok fixture-only {title!r}: {len(fixture)} chars")

    xml = (
        '<?xml version="1.0" encoding="UTF-8"?>\n<mediawiki>\n'
        + "\n".join(page_xml(p, r, t, s) for p, r, t, s in pages)
        + "\n</mediawiki>\n"
    )
    dump_path = DUMPS / "enwiki-20250101-pages-articles-multistream.xml.bz2"
    index_path = DUMPS / "enwiki-20250101-pages-articles-multistream-index.txt"
    dump_path.write_bytes(bz2.compress(xml.encode("utf-8")))
    index_path.write_text("\n".join(f"0:{p}:{t}" for p, _, t, _ in pages) + "\n", encoding="utf-8")
    print(f"dump: {dump_path}")
    print(f"index: {index_path}")
    print(f"pages: {len(pages)}")
    print(f"total_chars: {sum(len(s) for *_, s in pages)}")


if __name__ == "__main__":
    main()
