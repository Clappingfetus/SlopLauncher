import { describe, it, expect } from "vitest";
import type { Game, SmartGroup, SmartCondition } from "./types";
import { demoGames } from "./demo";
import { filterGames, mergeDiscovered } from "./library";
import { identity } from "./enrichment-identity";
import { groupGames, validateGroups } from "./grouping";
import { DAY, GIB, installedBytes, mainExtraHours } from "./game-facts";
import { gridLayout, gridRowAt } from "./grid-layout";
import { artworkRowsWindow } from "./artwork-window";
const now = Date.UTC(2026, 9, 21, 12);
const game = (id: string, hours?: number): Game => {
  const g = {
    ...demoGames()[0],
    id,
    title: id,
    favorite: false,
    hidden: false,
    providerIds: { hltb: 1 },
    addedAt: now - DAY,
    lastPlayed: now - 2 * DAY,
    enrichment: undefined,
  };
  return hours === undefined
    ? g
    : {
        ...g,
        enrichment: {
          hltb: {
            status: "ready",
            fingerprint: identity(g, "hltb"),
            checkedAt: now,
            nextCheckAt: now + DAY,
            data: { mainHours: 1, extraHours: hours },
          },
        },
      };
};
const rule = (
  field: SmartCondition["field"],
  op: string,
  value?: string,
  upper?: string,
): SmartCondition => ({ id: `${field}-${op}`, field, op, value, upper });
const group = (
  conditions: SmartCondition[],
  match: "all" | "any" = "all",
  id = "one",
): SmartGroup => ({ id, name: id, match, conditions });
describe("HLTB Main + Extras ordering", () => {
  it("sorts both ways with unknowns last, ties alphabetical, and no Main Story fallback", () => {
    const games = [
      game("Unknown"),
      game("Long", 80),
      game("Small", 5),
      game("Tie", 5),
      game("Zero", 0),
    ];
    expect(
      filterGames(games, "all", "", false, "", "hltb-asc").map((g) => g.id),
    ).toEqual(["Small", "Tie", "Long", "Unknown", "Zero"]);
    expect(
      filterGames(games, "all", "", false, "", "hltb-desc").map((g) => g.id),
    ).toEqual(["Long", "Small", "Tie", "Unknown", "Zero"]);
    expect(mainExtraHours(game("Invalid", NaN))).toBeUndefined();
  });
  it("ignores obsolete identity and immediately places newly enriched games", () => {
    const old = game("Old", 1),
      late = game("Late");
    expect(mainExtraHours({ ...old, title: "Rematched" })).toBeUndefined();
    expect(
      mainExtraHours({ ...old, providerIds: { hltb: 2 } }),
    ).toBeUndefined();
    expect(groupGames([old, late], "hltb").at(-1)?.name).toBe("No HLTB time");
    const updated = game("Late", 30);
    expect(groupGames([old, updated], "hltb").map((g) => g.name)).toEqual([
      "Under 10 h",
      "25–50 h",
    ]);
  });
});
describe("live smart groups", () => {
  it("combines tags/genres, installation size, release date, dates and Main + Extras", () => {
    const g = game("Match", 15);
    g.genres = ["RPG"];
    g.tags = ["Cozy"];
    g.year = 2020;
    g.copies = [{ ...g.copies[0], installSizeBytes: 5 * GIB }];
    const conditions = [
      rule("tags", "hasAll", "rpg,COZY"),
      rule("size", "between", "1", "10"),
      rule("release", "between", "2020-01-01", "2020-12-31"),
      rule("played", "within", "7"),
      rule("added", "within", "30"),
      rule("hltb", "lte", "20"),
    ];
    expect(groupGames([g], "smart", [group(conditions)], now)[0].name).toBe(
      "one",
    );
    expect(
      groupGames(
        [g],
        "smart",
        [group([...conditions, rule("hltb", "gt", "50")])],
        now,
      )[0].name,
    ).toBe("Other");
  });
  it("uses any/all, first-match priority, and retains unmatched/missing games exactly once", () => {
    const short = game("Short", 5),
      unknown = game("Missing");
    const rules = [
      group([rule("hltb", "lt", "10")], "all", "Quick"),
      group(
        [rule("hltb", "gte", "100"), rule("hltb", "unknown")],
        "any",
        "Unknown or long",
      ),
    ];
    const result = groupGames([short, unknown], "smart", rules, now);
    expect(result.map((g) => [g.name, g.games.map((g) => g.id)])).toEqual([
      ["Quick", ["Short"]],
      ["Unknown or long", ["Missing"]],
    ]);
    expect(
      groupGames(
        [short],
        "smart",
        [rules[0], group([rule("hltb", "known")], "all", "Also short")],
        now,
      ),
    ).toHaveLength(1);
    expect(
      groupGames(
        [unknown],
        "smart",
        [group([rule("size", "lt", "10")])],
        now,
      )[0].name,
    ).toBe("Other");
  });
  it("handles partial release years conservatively and exact days inclusively", () => {
    const g = game("Year");
    g.year = 2020;
    const match = (r: SmartCondition) =>
      groupGames([g], "smart", [group([r])], now)[0].name === "one";
    expect(match(rule("release", "before", "2020-06-01"))).toBe(false);
    expect(match(rule("release", "before", "2021-01-01"))).toBe(true);
    expect(match(rule("release", "between", "2020-01-01", "2020-12-31"))).toBe(
      true,
    );
    g.releasedAt = Date.UTC(2020, 4, 1);
    expect(match(rule("release", "between", "2020-05-01", "2020-05-01"))).toBe(
      true,
    );
  });
  it("ages relative groups and separates never played / unreported sizes", () => {
    const g = game("Recent");
    expect(groupGames([g], "played", [], now)[0].name).toBe("Last 7 days");
    expect(groupGames([g], "played", [], now + 40 * DAY)[0].name).toBe(
      "Earlier",
    );
    expect(
      groupGames([{ ...g, lastPlayed: undefined }], "played", [], now)[0].name,
    ).toBe("Never played");
    expect(groupGames([g], "size", [], now)[0].name).toBe("Size unknown");
  });
  it("uses the largest installed copy without summing duplicates or counting uninstalled copies", () => {
    const g = game("Copies");
    g.copies = [
      { ...g.copies[0], installSizeBytes: 5 * GIB },
      { ...g.copies[0], id: "two", installSizeBytes: 8 * GIB },
      {
        ...g.copies[0],
        id: "owned",
        installed: false,
        installSizeBytes: 100 * GIB,
      },
    ];
    expect(installedBytes(g)).toBe(8 * GIB);
    expect(groupGames([g], "size")[0].name).toBe("1–10 GiB");
    const scanned = mergeDiscovered(
      [g],
      [
        {
          ...g,
          copies: [
            {
              ...g.copies[0],
              installPath: "Z:/Moved",
              installSizeBytes: undefined,
            },
          ],
        },
      ],
    );
    expect(scanned[0].copies[0].installSizeBytes).toBeUndefined();
  });
  it("regroups rematches and late arrivals without changing hidden state, notes, favorites, or memberships", () => {
    const g = {
      ...game("First", 5),
      favorite: true,
      hidden: true,
      notes: "Keep",
    };
    const before = JSON.stringify(g);
    const late = game("Late", 50);
    expect(groupGames([g, late], "hltb").flatMap((g) => g.games)).toHaveLength(
      2,
    );
    expect(JSON.stringify(g)).toBe(before);
    expect(groupGames([{ ...g, title: "New identity" }], "hltb")[0].name).toBe(
      "No HLTB time",
    );
    expect(groupGames([], "tags")).toEqual([]);
  });
  it("rejects invalid/duplicate rules before saving", () => {
    expect(() =>
      validateGroups([group([rule("size", "between", "10", "1")])]),
    ).toThrow();
    expect(() =>
      validateGroups([group([rule("release", "before", "2020-02-31")])]),
    ).toThrow();
    expect(() =>
      validateGroups([group([rule("tags", "hasAny", "")])]),
    ).toThrow();
    expect(() =>
      validateGroups([
        group([rule("hltb", "known")]),
        group([rule("hltb", "unknown")]),
      ]),
    ).toThrow();
    expect(validateGroups([group([rule("hltb", "known")])])).toHaveLength(1);
  });
});
describe("one virtual viewport for grouped libraries", () => {
  it("lays out headers and partial rows, and finds scroll offsets with binary search", () => {
    const games = Array.from({ length: 11 }, (_, i) => game(String(i)));
    const groups = [
      { id: "a", name: "A", games: games.slice(0, 3) },
      { id: "b", name: "B", games: games.slice(3) },
    ];
    const layout = gridLayout(games, groups, 4, 300);
    expect(layout.rows.map((r) => r.top)).toEqual([0, 52, 352, 404, 704]);
    expect(layout.height).toBe(1004);
    expect(gridRowAt(layout.rows, 402)).toBe(2);
    expect(gridRowAt(layout.rows, 10000)).toBe(4);
    expect(layout.rows.flatMap((r) => r.games ?? []).map((g) => g.id)).toEqual(
      games.map((g) => g.id),
    );
  });
  it("bounds nearby image requests with 10,000 one-game groups", () => {
    const games = Array.from({ length: 10000 }, (_, i) => ({
      ...game(String(i)),
      cover: `art${i}`,
    }));
    const groups = games.map((g) => ({ id: g.id, name: g.title, games: [g] }));
    const layout = gridLayout(games, groups, 5, 320);
    const first = gridRowAt(layout.rows, 1000000),
      last = gridRowAt(layout.rows, 1000900) + 1;
    const requests = artworkRowsWindow(layout.rows, first, last, 1, 384);
    expect(requests.length).toBeLessThan(30);
    expect(requests.every((r) => Number(r.url.slice(3)) > 2500)).toBe(true);
    expect(requests.length * 384 * 384 * 6).toBeLessThan(48 * 1024 * 1024);
  });
});
