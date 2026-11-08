// web/src/features/poreia/components/poreia-catalog-sheet.test.tsx
import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import type { PoreiaCatalogSite } from "@/lib/entity-views";
import { PoreiaCatalogSheet } from "./poreia-catalog-sheet";

const site = (id: string, label: string): PoreiaCatalogSite => ({
  id,
  wikidata_qid: "Q64436",
  label,
  kind: "monument",
  visitability: "unknown",
  coordinates: { lat: 48.87, lon: 2.29 },
  historical_links: [{
    relation_role: "commissioned",
    relation_pid: "P88",
    subject_qid: "Q517",
    summary: "Commandé par Napoléon.",
    source_url: "https://www.wikidata.org/wiki/Q64436",
  }],
  media: [],
  source_url: "https://www.wikidata.org/wiki/Q64436",
});

test("the selected place card is rendered before the rest of the list", () => {
  const { container } = render(
    <PoreiaCatalogSheet
      sites={[site("later", "Plus tard"), site("chosen", "Arc choisi")]}
      occurrences={[]}
      selectedId="chosen"
      loading={false}
      hasMore={false}
      onSelect={() => undefined}
      onMore={() => undefined}
    />,
  );
  const card = container.querySelector(".poreia-place-card");
  const list = container.querySelector(".visit-heritage-list");
  expect(card).toBeTruthy();
  expect(list).toBeTruthy();
  expect(card!.compareDocumentPosition(list!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(screen.getByText("Commandé par Napoléon.")).toBeTruthy();
  expect(container.querySelector("a[href='https://www.wikidata.org/wiki/Q64436']")).toBeTruthy();
});
