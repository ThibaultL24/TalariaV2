// web/src/components/detail/source-ref-url.test.ts
import { expect, test } from "vitest";
import { resolveSourceParagraphHref } from "./source-ref-url";

const sentence = "He was the youngest son of Joseph Léopold Sigisbert Hugo, a general in the Napoleonic army";

test("a stored evidence label does not become a Wikipedia heading", () => {
  const href = resolveSourceParagraphHref({
    page_title: "Victor Hugo",
    language: "en",
    url: "https://en.wikipedia.org/wiki/Victor_Hugo",
    section_title: "evidence",
    quote: sentence,
  });
  expect(href).toContain("#:~:text=");
  expect(href).not.toContain("#evidence");
  expect(href).toContain(encodeURIComponent("He was the youngest son of Joseph"));
});

test("a real article section stays in front of the quoted sentence", () => {
  const href = resolveSourceParagraphHref({
    page_title: "Victor Hugo",
    section_title: "Early life",
    quote: sentence,
  });
  expect(href).toMatch(/#Early_life:~:text=/);
});

test("a structured row without a sentence stays on the page", () => {
  const href = resolveSourceParagraphHref({
    page_title: "Victor Hugo",
    url: "https://en.wikipedia.org/wiki/Victor_Hugo",
    section_title: "evidence",
    quote: "Victor Hugo | birth | born_in | 1802 |",
  });
  expect(href).toBe("https://en.wikipedia.org/wiki/Victor_Hugo");
});
