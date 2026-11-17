import { expect, it } from "vitest";
import { demoGames } from "./demo";
import { countGamesBySource } from "./source-counts";

it("counts each game once per source and reflects later imports and removals", () => {
  const first = demoGames()[0];
  const duplicate = { ...first.copies[0], id: "another-copy" };
  const multi = {
    ...first,
    copies: [
      ...first.copies,
      duplicate,
      { ...duplicate, id: "gog-copy", source: "gog" as const },
    ],
  };
  const late = { ...demoGames()[1], id: "late" };
  expect([...countGamesBySource([multi])]).toEqual([
    ["steam", 1],
    ["gog", 1],
  ]);
  expect(countGamesBySource([multi, late]).get("gog")).toBe(2);
  expect(countGamesBySource([late]).get("steam")).toBeUndefined();
});
