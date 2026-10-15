import { expect, test } from "vitest";
import { shortLifeRecap } from "./how-it-happened";

test("keeps at most three sentences and drops statement dumps", () => {
  expect(shortLifeRecap("STATEMENT\tbirth\t1769")).toBeNull();
  expect(
    shortLifeRecap("Hugo left for Guernsey. He wrote there. Friends visited. A fourth sentence stays out."),
  ).toBe("Hugo left for Guernsey. He wrote there. Friends visited.");
});
