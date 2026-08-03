#!/usr/bin/env python3
# scripts/seed_napoleon_dump.py — real Wikipedia multi-page corpus for dense Napoleon demos.
"""Fetch Wikipedia extracts and pack them into a synthetic multistream dump."""

from __future__ import annotations

import bz2
import json
import os
import urllib.parse
import urllib.request
from pathlib import Path

DATA_ROOT = Path(os.environ.get("TALARIA_DATA_ROOT", "/home/ubuntu/wiki-dump"))
DUMPS = DATA_ROOT / "dumps"
DUMPS.mkdir(parents=True, exist_ok=True)

UA = {"User-Agent": "TalariaEngine/0.1 (dense-napoleon-seed; research)"}

# Dense cultural corpus: biography + sentimental + diplomatic + military + residences.
TITLES = [
    "Napoleon",
    "Military career of Napoleon",
    "Joséphine de Beauharnais",
    "Marie Louise, Duchess of Parma",
    "Treaty of Amiens",
    "Treaties of Tilsit",
    "Continental System",
    "French Consulate",
    "First French Empire",
    "Hundred Days",
    "Napoleon II",
    "Battle of Austerlitz",
    "Battle of Waterloo",
    "Battle of Borodino",
    "Battle of Leipzig",
    "Battle of Jena–Auerstedt",
    "Battle of Marengo",
    "Battle of the Pyramids",
    "Battle of Wagram",
    "Battle of Friedland",
    "Coup of 18 Brumaire",
    "Concordat of 1801",
    "Napoleonic Code",
    "Congress of Erfurt",
    "Peninsular War",
    "French invasion of Russia",
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
        }
    )
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=60) as resp:
        data = json.load(resp)
    page = next(iter(data["query"]["pages"].values()))
    extract = page.get("extract")
    if not extract:
        return None
    resolved = page.get("title", title)
    page_id = int(page.get("pageid") or 0)
    return resolved, extract, page_id


def xml_escape(text: str) -> str:
    return (
        text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
    )


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


def main() -> None:
    pages: list[tuple[int, int, str, str]] = []
    seen_titles: set[str] = set()
    for i, title in enumerate(TITLES):
        got = fetch_extract(title)
        if not got:
            print(f"skip (empty): {title}")
            continue
        resolved, extract, page_id = got
        if resolved in seen_titles:
            print(f"skip (dup redirect): {title} -> {resolved}")
            continue
        seen_titles.add(resolved)
        pid = page_id or (2000 + i)
        pages.append((pid, 9000 + i, resolved, extract))
        print(f"ok {resolved!r}: {len(extract)} chars")

    xml = (
        '<?xml version="1.0" encoding="UTF-8"?>\n<mediawiki>\n'
        + "\n".join(page_xml(p, r, t, s) for p, r, t, s in pages)
        + "\n</mediawiki>\n"
    )
    dump_path = DUMPS / "enwiki-20250101-pages-articles-multistream.xml.bz2"
    index_path = DUMPS / "enwiki-20250101-pages-articles-multistream-index.txt"
    dump_path.write_bytes(bz2.compress(xml.encode("utf-8")))
    index_path.write_text(
        "\n".join(f"0:{p}:{t}" for p, _, t, _ in pages) + "\n", encoding="utf-8"
    )
    print(f"dump: {dump_path}")
    print(f"index: {index_path}")
    print(f"pages: {len(pages)}")
    print(f"total_chars: {sum(len(s) for *_, s in pages)}")


if __name__ == "__main__":
    main()
