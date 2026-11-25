# scripts/poreia/test_pilot_catalog.py
import unittest
from pathlib import Path

from build_pilot_seed import SQL, check, load_catalog


class PilotCatalogTest(unittest.TestCase):
    def test_catalog_stays_in_quarantine_and_matches_sql(self) -> None:
        errors = check(load_catalog(), SQL.read_text(encoding="utf-8"))
        self.assertEqual(errors, [])

    def test_foundation_is_not_described_as_a_visit(self) -> None:
        foundation = next(site for site in load_catalog()["sites"] if site["wikidata_qid"] == "Q17076145")
        self.assertEqual(foundation["relation_role"], "thematic")
        self.assertIn("1890", foundation["summary"])
        self.assertNotIn("a visité", foundation["summary"].lower())

    def test_sql_file_lives_outside_a_production_command(self) -> None:
        header = Path(SQL).read_text(encoding="utf-8").splitlines()[0]
        self.assertIn("quarantine", header.lower() + Path(SQL).read_text(encoding="utf-8")[:400].lower())


if __name__ == "__main__":
    unittest.main()
