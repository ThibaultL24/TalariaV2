// web/e2e/istoria-search.spec.ts
import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("talaria.locale", "fr");
  });
});

test("a typed name offers identities and does not start research by itself", async ({ page }) => {
  await page.goto("/istoria");
  const search = page.getByRole("combobox", { name: "Recherche de personnalité" });
  await search.fill("Dumas");
  const father = page.getByRole("option").filter({ hasText: "Q38337" });
  const son = page.getByRole("option").filter({ hasText: "Q169150" });
  await expect(father).toBeVisible();
  await expect(son).toBeVisible();
  await expect(page.getByRole("option").first()).not.toContainText("human settlement");
  await father.click();
  await expect(page.getByText("Identifiant retenu : Q38337")).toBeVisible();
  await expect(page.getByText("La recherche documentaire est fermée sur ce serveur")).toBeVisible();
  await expect(page.getByRole("button", { name: "Lancer la recherche" })).toHaveCount(0);
  await expect(page).toHaveURL(/\/istoria\/?$/);
});

test("a name with no Wikidata hit creates no fiche", async ({ page }) => {
  await page.goto("/istoria");
  const search = page.getByRole("combobox", { name: "Recherche de personnalité" });
  await search.fill("zzzznotapersonqqq");
  await page.getByRole("option").first().click();
  await expect(page.getByText("Aucun identifiant Wikidata fiable")).toBeVisible();
  await expect(page.getByRole("button", { name: "Lancer la recherche" })).toHaveCount(0);
  await expect(page).toHaveURL(/\/istoria\/?$/);
});

test("an absent person is not researched while writes are closed", async ({ page }) => {
  await page.goto("/istoria");
  const search = page.getByRole("combobox", { name: "Recherche de personnalité" });
  await search.fill("Marceline Desbordes-Valmore");
  await page.getByRole("option").filter({ hasText: "Q466987" }).first().click();
  await expect(page.getByText("Identifiant retenu : Q466987")).toBeVisible();
  await expect(page.getByText("La recherche documentaire est fermée sur ce serveur")).toBeVisible();
  await expect(page.getByRole("button", { name: "Lancer la recherche" })).toHaveCount(0);
  await expect(page).toHaveURL(/\/istoria\/?$/);
  const refused = await page.request.post("/api/v1/ingest/explorer", {
    data: { subject: "Marceline Desbordes-Valmore", qid: "Q466987", live: true },
  });
  expect(refused.status()).toBe(403);
});

test("Napoleon opens on the timeline, the map and the sources", async ({ page }) => {
  const health = await page.request.get("http://127.0.0.1:8081/health");
  const healthBody = await health.json();
  expect(healthBody.research_writes).toBe(false);
  expect(healthBody.research_qid).toBeNull();
  await page.goto("/istoria");
  const search = page.getByRole("combobox", { name: "Recherche de personnalité" });
  await search.fill("Napoleon");
  await page.getByRole("option").filter({ hasText: "Q517" }).first().click();
  await page.waitForURL(/\/istoria\/[0-9a-f-]{36}/i, { timeout: 60_000 });
  const entityUrl = page.url();
  await page.goto(entityUrl.replace(/\/[^/]*$/, "/timeline"));
  await expect(page.locator("canvas").first()).toBeVisible();
  await page.goto(entityUrl.replace(/\/[^/]*$/, "/map"));
  await expect(page.locator("canvas.maplibregl-canvas")).toBeVisible();
  await page.goto(entityUrl.replace(/\/[^/]*$/, "/sources"));
  await expect(page.getByRole("heading", { name: "Documents sources" })).toBeVisible();
  const enrich = page.getByRole("button", { name: "Enrichir le corpus" });
  await expect(enrich).toBeDisabled();
});
