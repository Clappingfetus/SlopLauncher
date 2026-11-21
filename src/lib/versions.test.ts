import { describe, it, expect } from "vitest";
import {
  versionLibrary,
  createVersionResolver,
  gameContextIdentity,
  versionOptions,
} from "./versions";
import { filterGames, preferredCopy } from "./library";
import { identity } from "./enrichment-identity";
import type { Game, Settings, Source } from "./types";
import { defaultSettings } from "./types";
import {
  changeDuplicateFlag,
  changeDuplicateFlags,
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
  it("counts groups after additions, rematches and removals, including filtered lists", () => {
    const first = game("1", "steam"), late = game("2", "gog");
    const renamed = { ...late, title: "Different Adventure" };
    for (const [games, expected] of [
      [[first], 1], [[first, late], 1], [[first, renamed], 2], [[renamed], 1],
    ] as const) {
      const list = [...games];
      const index = versionLibrary(list, settings);
      expect(index.count(list)).toBe(expected);
      expect(index.count(list.slice(1))).toBe(index.collapse(list.slice(1)).length);
      expect(versionLibrary(list, settings, false).count(list)).toBe(list.length);
    }
  });
  it("counts hidden, favorite and installed entries independently across copy groups", () => {
    const visible = game("1", "epic");
    const hidden = { ...game("2", "gog"), hidden: true, favorite: true };
    const standalone = { ...game("3", "steam", "Separate Adventure"), favorite: true };
    const rematched = { ...hidden, title: "Island Adventure" };
    for (const [list, expected] of [
      [[visible, standalone], { library: 2, hidden: 0, favorites: 1, installed: 1 }],
      [[visible, standalone, hidden], { library: 2, hidden: 1, favorites: 2, installed: 2 }],
      [[visible, standalone, rematched], { library: 3, hidden: 1, favorites: 2, installed: 2 }],
      [[standalone, rematched], { library: 2, hidden: 1, favorites: 2, installed: 2 }],
    ] as const) {
      const games = [...list];
      const index = versionLibrary(games, settings);
      expect(index.counts(games, true)).toEqual(expected);
      const shown = games.filter((entry) => !entry.hidden);
      expect(index.counts(games, false)).toEqual({
        library: shown.length,
        hidden: expected.hidden,
        favorites: 1,
        installed: 1,
      });
      const separate = versionLibrary(games, { ...settings, hideDuplicateGames: false });
      expect(separate.counts(games, true)).toEqual({ ...expected, library: games.length });
    }
  });
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
  it("does not carry a removed member's catalog facts into the next index", () => {
    const resolve = createVersionResolver();
    const steam = game("1", "steam");
    const original = game("2", "gog");
    const replacement = game("3", "gog");
    expect(resolve([steam, original], settings).members(steam.id)).toEqual([
      steam,
      original,
    ]);
    expect(resolve([steam, replacement], settings).members(steam.id)).toEqual([
      steam,
      replacement,
    ]);
  });
  it("identifies same-source choices by path and scopes copy IDs to their game", () => {
    const first = game("1", "manual");
    const second = game("2", "manual");
    const a = { ...first.copies[0], id: "local", executable: "C:/QA/First.exe" };
    const b = { ...second.copies[0], id: "local", executable: "C:/QA/Second.exe" };
    const options = versionOptions([
      { game: first, copy: a },
      { game: second, copy: b },
      { game: second, copy: { ...b, id: "offline", available: false } },
    ]);
    expect(options.map(({ label }) => label)).toEqual([
      "First.exe · Manually added · Installed",
      "Second.exe · Manually added · Installed",
      "Manually added · Unavailable",
    ]);
    expect(new Set(options.map(({ key }) => key)).size).toBe(3);
    const sharedFilename = versionOptions([
      { game: first, copy: a },
      { game: second, copy: { ...b, executable: "D:\\Backup\\First.exe" } },
    ]);
    expect(sharedFilename.map(({ label }) => label)).toEqual([
      "First.exe · Manually added · Installed · C:/QA/First.exe",
      "First.exe · Manually added · Installed · D:\\Backup\\First.exe",
    ]);
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
  it("advances each selected group's flag clock once, preserving the choice on late rematches", () => {
    const steam = {
      ...game("1", "steam"),
      favorite: true,
      favoriteUpdatedAt: 200,
    };
    const gog = { ...game("2", "gog"), favorite: true, favoriteUpdatedAt: 201 };
    const other = game("3", "steam", "Different");
    const games = [steam, gog, other];
    const changed = changeDuplicateFlags(
      games,
      index(games),
      [steam.id, gog.id, "removed", other.id],
      "favorite",
      false,
      100,
    );
    expect(changed.updates).toEqual([
      { id: steam.id, favorite: false, favoriteUpdatedAt: 202 },
      { id: gog.id, favorite: false, favoriteUpdatedAt: 202 },
      { id: other.id, favorite: false, favoriteUpdatedAt: 100 },
    ]);
    expect(changed.games[1].notes).toBe(gog.notes);
    expect(changed.games[1].copies).toBe(gog.copies);

    const late = {
      ...game("4", "epic", "Unmatched"),
      favorite: true,
      favoriteUpdatedAt: 201,
    };
    const unmatched = [...changed.games, late];
    expect(reconcileDuplicateFlags(unmatched, index(unmatched)).games).toBe(
      unmatched,
    );
    const rematched = [...changed.games, { ...late, title: steam.title }];
    const reconciled = reconcileDuplicateFlags(rematched, index(rematched)).games;
    expect(reconciled[3]).toMatchObject({
      notes: late.notes,
      favorite: false,
      favoriteUpdatedAt: 202,
    });
    const remaining = reconciled.slice(1);
    expect(reconcileDuplicateFlags(remaining, index(remaining)).games).toBe(
      remaining,
    );
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
