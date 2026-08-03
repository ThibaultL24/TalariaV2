#!/usr/bin/env python3
# scripts/seed_napoleon_dump.py — dense synthetic Wikipedia multistream dump for Napoleon demos.
"""Build a small bz2 multistream dump + index rich in life-event sentences."""

from __future__ import annotations

import bz2
import os
from pathlib import Path

DATA_ROOT = Path(os.environ.get("TALARIA_DATA_ROOT", "/home/ubuntu/wiki-dump"))
DUMPS = DATA_ROOT / "dumps"
DUMPS.mkdir(parents=True, exist_ok=True)

# Dense biography-style sentences matching mock:life_events patterns.
# Cultural facts only — opinions/théories go to claims/Intuition separately.
SENTENCES = [
    "Napoleon Bonaparte was born in 1769 in Ajaccio.",
    "Napoleon Bonaparte studied at Brienne in 1784.",
    "Napoleon Bonaparte fought at Toulon in 1793.",
    "Napoleon Bonaparte visited Egypt in 1798.",
    "Napoleon Bonaparte fought at Marengo in 1800.",
    "Napoleon Bonaparte was appointed consul in 1799 in Paris.",
    "Napoleon Bonaparte married Josephine in 1796 in Paris.",
    "Napoleon Bonaparte was crowned in 1804 in Paris.",
    "Napoleon Bonaparte fought at Austerlitz in 1805.",
    "Napoleon Bonaparte fought at Jena in 1806.",
    "Napoleon Bonaparte fought at Wagram in 1809.",
    "Napoleon Bonaparte invaded Russia in 1812.",
    "Napoleon Bonaparte visited Moscow in 1812.",
    "Napoleon Bonaparte lived in Fontainebleau in 1814.",
    "Napoleon Bonaparte was exiled to Elba in 1814.",
    "Napoleon Bonaparte fought at Waterloo in 1815.",
    "Napoleon Bonaparte was exiled to Saint Helena in 1815.",
    "Napoleon Bonaparte lived in Saint Helena in 1815.",
    "Napoleon Bonaparte died in 1821 in Saint Helena.",
    "Napoleon Bonaparte fought at Leipzig in 1813.",
    "Napoleon Bonaparte resided in Malmaison in 1800.",
    "Napoleon Bonaparte moved to Paris in 1792.",
    "Napoleon Bonaparte served as general in 1796 in Milan.",
    "A statue of Napoleon Bonaparte was unveiled in 1865 in Paris.",
    # Related pages for multi-document density
]

RELATED = {
    "Josephine de Beauharnais": [
        "Josephine de Beauharnais was born in 1763 in Paris.",
        "Josephine de Beauharnais married Napoleon Bonaparte in 1796 in Paris.",
        "Josephine de Beauharnais lived in Malmaison in 1800.",
        "Josephine de Beauharnais died in 1814 in Malmaison.",
    ],
    "Battle of Waterloo": [
        "Napoleon Bonaparte fought at Waterloo in 1815.",
        "Arthur Wellesley fought at Waterloo in 1815.",
    ],
    "Battle of Austerlitz": [
        "Napoleon Bonaparte fought at Austerlitz in 1805.",
    ],
    "Ajaccio": [
        "Napoleon Bonaparte was born in 1769 in Ajaccio.",
    ],
}


def page_xml(page_id: int, rev_id: int, title: str, sentences: list[str]) -> str:
    text = " ".join(sentences)
    return f"""  <page>
    <title>{title}</title>
    <ns>0</ns>
    <id>{page_id}</id>
    <revision>
      <id>{rev_id}</id>
      <text>{text}</text>
    </revision>
  </page>"""


def main() -> None:
    pages: list[tuple[int, int, str, list[str]]] = [
        (1001, 5001, "Napoleon", SENTENCES),
        (1002, 5002, "Napoleon Bonaparte", SENTENCES),
    ]
    pid = 1003
    rid = 5003
    for title, sents in RELATED.items():
        pages.append((pid, rid, title, sents))
        pid += 1
        rid += 1

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
    print(f"napoleon_sentences: {len(SENTENCES)}")


if __name__ == "__main__":
    main()
