import { describe, it, expect } from "vitest";
import {
  versionLibrary,
  createVersionResolver,
  gameContextIdentity,
} from "./versions";
import { filterGames, preferredCopy } from "./library";
import { identity } from "./enrichment-identity";
import type { Game, Settings, Source } from "./types";
import { defaultSettings } from "./types";
import {
  changeDuplicateFlag,
  reconcileDuplicateFlags,
} from "./duplicate-flags";
const game = (
  id: string,
  source: Source,
  title = "Shared Adventure",
  year = 2024,
): Game => ({
  id,
  title,
  copies: [
    {
      id: `${source}:${id}`,
      source,
      sourceId: id,
      installed: source !== "epic",
    },
  ],
  year,
  genres: [],
  screenshots: [],
  favorite: false,
  notes: `Notes ${id}`,
  status: "unplayed",
  addedAt: 1,
});
const settings: Settings = {
  ...defaultSettings,
  hideDuplicateGames: true,
  preferredSource: "gog",
};
describe("reversible duplicate versions", () => {
  it("matches punctuation and ampersand variants without stripping edition or sequel words", () => {
    const games = [
      game("1", "steam", "Hero’s Quest™: Fire & Ice"),
      game("2", "gog", "Heros Quest - Fire and Ice"),
      game("3", "epic", "Heros Quest Fire and Ice Deluxe"),
    ];
    expect(
      versionLibrary(games, settings)
        .members("1")
        .map((g) => g.id),
    ).toEqual(["1", "2"]);
  });
  it("uses native catalog IDs across renames and rejects conflicting metadata and transitive bridges", () => {
    const a = game("1", "gog", "Old Name");
    const b = game("2", "gog", "New Name");
    b.copies[0].sourceId = a.copies[0].sourceId;
    expect(versionLibrary([a, b], settings).members("1")).toHaveLength(2);
    const steam = { ...game("100", "steam"), metadataId: 200 };
    const other = { ...game("200", "steam"), providerIds: { igdb: 2 } };
    expect(
      versionLibrary([steam, other], settings).members("100"),
    ).toHaveLength(1);
    const catalogA = {
      ...game("11", "steam", "Alias A"),
      providerIds: { igdb: 1 },
    };
    const bridge = {
      ...game("22", "gog", "Alias B"),
      providerIds: { igdb: 1 },
    };
    const catalogB = {
      ...game("33", "epic", "Alias B"),
      providerIds: { igdb: 2 },
    };
    for (const list of [
      [catalogA, bridge, catalogB],
      [catalogB, bridge, catalogA],
    ])
      expect(versionLibrary(list, settings).members("33")).toHaveLength(1);
  });
  it("shows one preferred platform but leaves originals, notes and collections untouched", () => {
    const games = [game("1", "steam"), game("2", "gog"), game("3", "epic")],
      before = structuredClone(games),
      v = versionLibrary(games, settings);
    expect(v.collapse(games).map((g) => g.id)).toEqual(["2"]);
    expect(v.versions("1")).toHaveLength(3);
    expect(games).toEqual(before);
    expect(
      versionLibrary(games, {
        ...settings,
        hideDuplicateGames: false,
      }).collapse(games),
    ).toEqual(games);
  });
  it("persists manual choices, keeps installed fallback, ignores missing targets and respects filtered scopes", () => {
    const games = [game("1", "steam"), game("2", "gog"), game("3", "epic")];
    const prefs = { "1": { gameId: "3", copyId: "epic:3", selectedAt: 3 } };
    expect(
      versionLibrary(games, {
        ...settings,
        versionPreferences: prefs,
      }).collapse(games)[0].id,
    ).toBe("3");
    expect(
      versionLibrary(games, {
        ...settings,
        versionPreferences: prefs,
      }).collapse([games[0]])[0].id,
    ).toBe("1");
    expect(
      versionLibrary(games.slice(0, 2), {
        ...settings,
        preferredSource: "epic",
        versionPreferences: prefs,
      }).collapse(games.slice(0, 2))[0].copies[0].installed,
    ).toBe(true);
  });
  it("keeps editions, contradictory years/catalog matches, ambiguous same-store titles, and console systems separate", () => {
    expect(
      versionLibrary(
        [game("1", "steam"), game("2", "gog", "Shared Adventure Deluxe")],
        settings,
      ).collapse([
        game("1", "steam"),
        game("2", "gog", "Shared Adventure Deluxe"),
      ]),
    ).toHaveLength(2);
    const years = [
      game("1", "steam"),
      game("2", "gog", "Shared Adventure", 1993),
    ];
    expect(versionLibrary(years, settings).collapse(years)).toHaveLength(2);
    const sameStore = [game("1", "steam"), game("2", "steam")];
    expect(
      versionLibrary(sameStore, settings).collapse(sameStore),
    ).toHaveLength(2);
    const consoles = [
      game("1", "steam"),
      game("2", "emulated"),
      game("3", "emulated"),
    ];
    consoles[1].copies[0].emulation = {
      platform: "ps1",
      profileId: "one",
      gamePath: "one.chd",
    };
    consoles[2].copies[0].emulation = {
      platform: "ps2",
      profileId: "two",
      gamePath: "two.iso",
    };
    expect(versionLibrary(consoles, settings).collapse(consoles)).toHaveLength(
      3,
    );
    const conflicts = [
      { ...game("1", "steam"), providerIds: { igdb: 100 } },
      { ...game("2", "gog"), providerIds: { igdb: 200 } },
    ];
    expect(
      versionLibrary(conflicts, settings).collapse(conflicts),
    ).toHaveLength(2);
  });
  it("uses current catalog identities, and updates for late additions, renames, rematches and removals", () => {
    const resolve = createVersionResolver();
    let games = [{ ...game("1", "steam"), providerIds: { igdb: 100 } }];
    expect(resolve(games, settings).collapse(games)).toHaveLength(1);
    games = [
      ...games,
      { ...game("2", "gog", "Other store name"), providerIds: { igdb: 100 } },
    ];
    expect(resolve(games, settings).collapse(games)).toHaveLength(1);
    games = games.map((g) =>
      g.id === "2" ? { ...g, providerIds: { igdb: 200 } } : g,
    );
    expect(resolve(games, settings).collapse(games)).toHaveLength(2);
    expect(resolve(games.slice(1), settings).versions("1")).toEqual([]);
  });
  it("honors multi-copy choices without changing enrichment fingerprints and reuses singleton game objects", () => {
    const g = game("1", "steam");
    g.copies.push({ ...game("2", "gog").copies[0] });
    const key = identity(g, "pcgw"),
      v = versionLibrary([g], settings),
      shown = v.collapse([g])[0];
    expect(preferredCopy(shown)?.source).toBe("gog");
    expect(identity(shown, "pcgw")).toBe(key);
    const single = game("only", "steam");
    expect(versionLibrary([single], settings).collapse([single])[0]).toBe(
      single,
    );
  });
  it("does not turn a 10000-game same-store title collision into a quadratic search", () => {
    const games = Array.from({ length: 10000 }, (_, i) =>
      game(String(i), "steam"),
    );
    expect(versionLibrary(games, settings).collapse(games)).toHaveLength(10000);
  });
});

describe("shared duplicate preferences", () => {
  it("does not assign an ambiguous catalog match to the first conflicting store copy", () => {
    const games = [
      { ...game("100", "steam"), providerIds: { igdb: 1 } },
      { ...game("200", "steam"), providerIds: { igdb: 1 } },
      { ...game("300", "epic"), providerIds: { igdb: 1 } },
    ];
    for (const order of [games, [...games].reverse()]) {
      expect(versionLibrary(order, settings).members("300")).toHaveLength(1);
      const changed = changeDuplicateFlag(
        order,
        versionLibrary(order, settings),
        "300",
        "hidden",
        true,
      );
      expect(changed.updates.map((update) => update.id)).toEqual(["300"]);
    }
  });
  const index = (games: Game[]) =>
    versionLibrary(games, { ...settings, hideDuplicateGames: false });
  it("applies both flags across sources with duplicate hiding off and persists explicit false for late imports", () => {
    let games = [game("1", "steam"), game("2", "gog")];
    const note = games[1].notes;
    games = changeDuplicateFlag(
      games,
      index(games),
      "2",
      "favorite",
      true,
      100,
    ).games;
    games = changeDuplicateFlag(
      games,
      index(games),
      "1",
      "hidden",
      true,
      101,
    ).games;
    expect(games.every((g) => g.hidden && g.favorite)).toBe(true);
    games = changeDuplicateFlag(
      games,
      index(games),
      "2",
      "hidden",
      false,
      102,
    ).games;
    games = changeDuplicateFlag(
      games,
      index(games),
      "1",
      "favorite",
      false,
      103,
    ).games;
    games = JSON.parse(JSON.stringify(games));
    games.push({ ...game("3", "epic"), hidden: true, favorite: true });
    games = reconcileDuplicateFlags(games, index(games)).games;
    expect(games.every((g) => !g.hidden && !g.favorite)).toBe(true);
    expect(games[2].favoriteUpdatedAt).toBe(103);
    expect(games[1].notes).toBe(note);
    expect(reconcileDuplicateFlags(games, index(games)).games).toBe(games);
  });
  it("retains legacy positive flags, inherits on late matching, and stops sharing after rematching or removal", () => {
    let games = [
      { ...game("1", "steam"), favorite: true },
      game("2", "gog", "Different"),
    ];
    expect(reconcileDuplicateFlags(games, index(games)).updates).toEqual([]);
    games[1] = { ...games[1], title: games[0].title };
    games = reconcileDuplicateFlags(games, index(games)).games;
    expect(games[1].favorite).toBe(true);
    games[1] = { ...games[1], title: "Other identity" };
    games = changeDuplicateFlag(
      games,
      index(games),
      "1",
      "favorite",
      false,
      200,
    ).games;
    expect(games[1].favorite).toBe(true);
    expect(
      changeDuplicateFlag(
        games.slice(1),
        index(games.slice(1)),
        "1",
        "hidden",
        true,
      ).updates,
    ).toEqual([]);
  });
  it("reconciles a large library idempotently without replacing unchanged objects", () => {
    const games = Array.from({ length: 10000 }, (_, i) =>
      game(String(i), "steam", `Title ${i}`),
    );
    expect(reconcileDuplicateFlags(games, index(games)).games).toBe(games);
  });
});

it("invalidates an open context target when a copy changes or disappears", () => {
  const g = game("context", "steam"),
    key = gameContextIdentity(g, g.copies[0]);
  expect(gameContextIdentity({ ...g, notes: "Edited" }, g.copies[0])).toBe(key);
  expect(gameContextIdentity(g, { ...g.copies[0], installed: false })).not.toBe(
    key,
  );
  expect(gameContextIdentity(g, undefined)).not.toBe(key);
  expect(
    gameContextIdentity({ ...g, title: "Rematched" }, g.copies[0]),
  ).not.toBe(key);
});

describe("hidden library duplicate presentation", () => {
  it("collapses hidden copies, handles late additions and rematches, and retains originals", () => {
    const steam = { ...game("hs", "steam"), hidden: true };
    const gog = { ...game("hg", "gog"), hidden: true };
    const visible = game("visible", "epic", "Visible game");
    const rows = (games: Game[]) =>
      versionLibrary(games, settings).collapse(
        filterGames(games, "hidden", "", false, "", "title", [], false),
      );
    expect(rows([steam, visible])).toHaveLength(1);
    expect(rows([steam, gog, visible]).map((g) => g.id)).toEqual(["hg"]);
    expect(
      rows([steam, { ...gog, title: "Different game" }, visible]),
    ).toHaveLength(2);
    expect(rows([steam, visible])).toHaveLength(1);
    expect(steam.hidden).toBe(true);
    expect(gog.notes).toBe("Notes hg");
    expect(
      versionLibrary([steam, gog], {
        ...settings,
        hideDuplicateGames: false,
      }).collapse([steam, gog]),
    ).toHaveLength(2);
  });
});

it("reuses version lists within an index and refreshes for late copies and rematches", () => {
  const a = game("cache-a", "steam"),
    b = game("cache-b", "gog");
  const first = versionLibrary([a, b], settings);
  expect(first.versions(a.id)).toBe(first.versions(b.id));
  expect(first.versions(a.id)).toHaveLength(2);
  const late = game("cache-c", "epic");
  const next = versionLibrary([a, b, late], settings);
  expect(next.versions(a.id)).toHaveLength(3);
  expect(next.versions(a.id)).not.toBe(first.versions(a.id));
  const rematched = versionLibrary(
    [a, { ...b, title: "Other game" }],
    settings,
  );
  expect(rematched.versions(a.id)).toHaveLength(1);
  expect(rematched.versions(late.id)).toEqual([]);
});
