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
  await expect(page.getByRole("button", { name: "Lancer la recherche" })).toBeEnabled();
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

test("selecting Desbordes-Valmore starts a bounded research and keeps the fiche", async ({ page }) => {
  test.skip(test.info().project.name === "mobile", "The research itself is exercised once, on desktop.");
  await page.goto("/istoria");
  const search = page.getByRole("combobox", { name: "Recherche de personnalité" });
  await search.fill("Marceline Desbordes-Valmore");
  await page.getByRole("option").filter({ hasText: "Q466987" }).first().click();
  const research = page.getByRole("button", { name: "Lancer la recherche" });
  if (await research.isVisible()) {
    await expect(page.getByText("Identifiant retenu : Q466987")).toBeVisible();
    await research.click();
    await expect(page.getByText(/Recherche en cours ·/)).toBeVisible();
  }
  await page.waitForURL(/\/istoria\/[0-9a-f-]{36}/i, { timeout: 120_000 });
  const entityUrl = page.url();

  await page.goto(entityUrl.replace(/\/[^/]*$/, "/timeline"));
  await expect(page.locator("canvas").first()).toBeVisible();
  await page.goto(entityUrl.replace(/\/[^/]*$/, "/map"));
  await expect(page.locator("canvas.maplibregl-canvas")).toBeVisible();

  await page.reload();
  await expect(page.getByRole("button", { name: "Enrichir le corpus" })).toBeVisible();
  await expect(page.getByText(/^\d+%$/)).toHaveCount(0);
});
