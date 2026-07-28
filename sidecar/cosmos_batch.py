#!/usr/bin/env python3
# sidecar/cosmos_batch.py
"""Batch COSMOS tuple extraction for Talaria Engine (JSON in/out)."""

from __future__ import annotations

import argparse
import json
import os
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
COSMOS_ROOT = os.path.join(ROOT, "cosmos")
if os.path.isdir(COSMOS_ROOT):
    sys.path.insert(0, COSMOS_ROOT)

from preprocessing.tuple_extraction import (  # noqa: E402
    extract_loc,
    extract_person,
    extract_subjects,
    extract_verb,
    find_pv_match,
    find_vl_match,
    find_vt_match,
    generate_timex_tag_list,
    get_nltk_tree,
    is_target_sent,
    merge_by_verb,
    nlp,
)


def extract_with_verb(sent) -> list[dict]:
    verb_list = extract_verb(sent)
    person_list = {**extract_person(sent), **{"subject": extract_subjects(sent)}}
    loc_list = extract_loc(sent)
    time_list = generate_timex_tag_list(sent)
    nltk_tree = get_nltk_tree(sent)

    person_match_verb, _, _ = find_pv_match(nltk_tree, person_list, verb_list)
    verb_match_time, _, _ = find_vt_match(nltk_tree, verb_list, time_list)
    verb_match_loc, _, _ = find_vl_match(nltk_tree, verb_list, loc_list)
    extracted = merge_by_verb(person_match_verb, verb_match_time, verb_match_loc)

    tuples: list[dict] = []
    for person, verb, place, time in extracted:
        if None in (person, time, place):
            continue
        tuples.append(
            {
                "person": person,
                "time": time,
                "place": place,
                "verb": verb,
            }
        )
    return tuples


def process_sentence(text: str) -> list[dict]:
    doc = nlp(text)
    tuples: list[dict] = []
    for sent in doc.sents:
        if not is_target_sent(sent):
            continue
        tuples.extend(extract_with_verb(sent))
    return tuples


def main() -> None:
    parser = argparse.ArgumentParser(description="COSMOS batch tuple extraction")
    parser.add_argument("--input", required=True, help="Input JSON array [{id, text}, ...]")
    parser.add_argument("--output", required=True, help="Output JSON array")
    args = parser.parse_args()

    with open(args.input, encoding="utf-8") as handle:
        items = json.load(handle)

    results = []
    for item in items:
        sentence_id = item["id"]
        text = item["text"]
        tuples = process_sentence(text)
        results.append({"id": sentence_id, "tuples": tuples})

    with open(args.output, "w", encoding="utf-8") as handle:
        json.dump(results, handle, ensure_ascii=False)


if __name__ == "__main__":
    main()
