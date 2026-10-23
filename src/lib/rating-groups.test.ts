import { expect, it } from "vitest";
import { groupGames, validateGroups, usesRatings } from "./grouping";
import { collectionMembers, saveCollection } from "./collections";
import { identity } from "./enrichment-identity";
import type { Game, SmartGroup } from "./types";
const game = (id: string, score?: number): Game => {
  const g: Game = {
    id,
    title: id,
    copies: [],
    genres: ["RPG"],
    screenshots: [],
    favorite: false,
    status: "unplayed",
    notes: "",
    addedAt: 1,
  };
  if (score !== undefined)
    g.enrichment = {
      "reviews-metacritic": {
        fingerprint: identity(g, "reviews-metacritic"),
        status: "ready",
        checkedAt: 1,
        nextCheckAt: 999999,
        data: {
          rating: {
            source: "metacritic",
            kind: "users",
            value: score,
            scale: 10,
            count: 100,
            url: "https://www.metacritic.com/game/test/",
          },
        },
      },
    };
  return g;
};
const rules: SmartGroup[] = [
  {
    id: "best",
    name: "Highly rated RPGs",
    match: "all",
    conditions: [
      {
        id: "score",
        field: "rating",
        op: "between",
        value: "8.5",
        upper: "10",
      },
      { id: "tag", field: "tags", op: "hasAny", value: "RPG" },
    ],
  },
];
it("combines precise consensus thresholds with tags and handles late ratings and rematches", () => {
  expect(validateGroups(rules)).toEqual(rules);
  const a = game("a", 8.49),
    b = game("b", 8.5),
    late = game("late");
  expect(
    groupGames([a, b, late], "smart", rules)[0].games.map((g) => g.id),
  ).toEqual(["b"]);
  expect(
    groupGames([a, b, game("late", 9)], "smart", rules)[0].games.map(
      (g) => g.id,
    ),
  ).toEqual(["b", "late"]);
  expect(
    groupGames([{ ...b, title: "Different identity" }], "smart", rules)[0].id,
  ).toBe("other");
  expect(usesRatings("smart", rules)).toBe(true);
  expect(usesRatings("rating")).toBe(true);
  expect(usesRatings("tags", rules)).toBe(false);
});
it("keeps unrated games separate, orders score bands high first, and supports presence rules", () => {
  const games = [
    game("none"),
    game("low", 2),
    game("eight", 8),
    game("nine", 9),
    game("seven", 7),
    game("six", 6),
    game("ten", 10),
  ];
  expect(groupGames(games, "rating").map((g) => g.id)).toEqual([
    "rating:0",
    "rating:1",
    "rating:2",
    "rating:3",
    "rating:4",
    "rating:5",
  ]);
  const unknown: SmartGroup[] = [
    {
      ...rules[0],
      conditions: [{ id: "unknown", field: "rating", op: "unknown" }],
    },
  ];
  expect(groupGames(games, "smart", unknown)[0].games.map((g) => g.id)).toEqual(
    ["none"],
  );
});
it("validates the 1–10 range and preserves dynamic collection rules through restart", () => {
  for (const value of ["0", "10.5", "NaN", "-1"]) {
    const bad = structuredClone(rules);
    bad[0].conditions[0].value = value;
    expect(() => validateGroups(bad)).toThrow();
  }
  const bad = structuredClone(rules);
  bad[0].conditions[0].upper = "11";
  expect(() => validateGroups(bad)).toThrow();
  const saved = saveCollection(
    [],
    {
      id: "rated",
      name: "Highly rated RPGs",
      gameIds: [],
      rule: { mode: "smart", groupId: "smart:best", groups: rules },
    },
    [],
  );
  const restored = JSON.parse(JSON.stringify(saved))[0];
  expect(
    collectionMembers(restored, [game("new", 9), game("unrated")]),
  ).toEqual(["new"]);
  expect(collectionMembers(restored, [])).toEqual([]);
});
