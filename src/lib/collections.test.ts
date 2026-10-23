import { describe, it, expect } from "vitest";
import {
  collectionCounts,
  resolveCollections,
  collectionMembers,
  saveCollection,
  setMembership,
  withoutGame,
} from "./collections";
import { filterGames, mergeDiscovered, mergeOwnedLibrary } from "./library";
import { demoGames } from "./demo";
import { identity } from "./enrichment-identity";
import type { GameCollection } from "./types";
describe("custom collections follow the live library", () => {
  it("accepts games added later across sources and combines search, tags, installed state and sorting", () => {
    const games = demoGames().slice(0, 2);
    let collections: GameCollection[] = [];
    collections = saveCollection(
      collections,
      { id: "weekend", name: " Weekend ", gameIds: [games[0].id] },
      games,
    );
    const late = {
      ...structuredClone(games[1]),
      id: "future:game",
      title: "Future Adventure",
      tags: ["Co-op"],
      copies: [{ ...games[1].copies[0], id: "future-copy", installed: false }],
    };
    games.push(late);
    collections = setMembership(collections, "weekend", late.id, true);
    expect(
      filterGames(
        games,
        "collection:weekend",
        "future",
        false,
        "co-op",
        "title",
        collections,
      ).map((g) => g.id),
    ).toEqual([late.id]);
    expect(
      filterGames(
        games,
        "collection:weekend",
        "future",
        true,
        "",
        "title",
        collections,
      ),
    ).toEqual([]);
    expect(collectionCounts(collections, games).get("weekend")).toBe(2);
    expect(
      filterGames(
        games,
        "collection:deleted",
        "",
        false,
        "",
        "title",
        collections,
      ),
    ).toEqual([]);
  });
  it("membership survives renames, rematches, rescans, owned imports and file moves", () => {
    const game = demoGames()[0];
    game.notes = "Keep notes";
    game.favorite = true;
    const collections = [
      { id: "folder", name: "Favorites to finish", gameIds: [game.id] },
    ];
    const incoming = structuredClone(game);
    incoming.title = "Renamed by source";
    incoming.copies[0].installPath = "E:\\Moved";
    let games = mergeDiscovered([game], [incoming]);
    games[0] = { ...games[0], title: "My new title", metadataId: 555 };
    const owned = structuredClone(incoming);
    owned.copies[0].source = "steam";
    owned.copies[0].ownership = { accountId: "owner", verifiedAt: 1 };
    games = mergeOwnedLibrary(games, [owned], "steam");
    expect(
      filterGames(
        games,
        "collection:folder",
        "",
        false,
        "",
        "title",
        collections,
      )[0],
    ).toMatchObject({
      id: game.id,
      title: "My new title",
      notes: "Keep notes",
      favorite: true,
    });
    expect(games[0].copies[0].installPath).toBe("E:\\Moved");
  });
  it("allows multiple folders, deduplicates membership and removes only folder references", () => {
    const games = demoGames();
    let collections = [
      { id: "a", name: "A", gameIds: [games[0].id] },
      { id: "b", name: "B", gameIds: [] },
    ];
    collections = setMembership(collections, "b", games[0].id, true);
    collections = setMembership(collections, "b", games[0].id, true);
    expect(collections[1].gameIds).toEqual([games[0].id]);
    collections = setMembership(collections, "a", games[0].id, false);
    expect(collections[1].gameIds).toHaveLength(1);
    expect(
      withoutGame(collections, games[0].id).every((c) => !c.gameIds.length),
    ).toBe(true);
    expect(games).toHaveLength(demoGames().length);
  });
  it("ignores disappeared games in counts and editor saves, retaining concurrently added games", () => {
    const games = demoGames();
    const initial = [{ id: "a", name: "A", gameIds: [games[0].id, "removed"] }];
    expect(collectionCounts(initial, games).get("a")).toBe(1);
    const saved = saveCollection(
      initial,
      {
        ...initial[0],
        name: "Renamed",
        gameIds: [games[0].id, games[1].id, "removed", games[1].id],
      },
      games,
    );
    expect(saved[0].gameIds).toEqual([games[0].id, games[1].id]);
    expect(initial[0].name).toBe("A");
  });
  it("validates names and permits renaming a collection to its existing name", () => {
    const original = [{ id: "a", name: "Weekend", gameIds: [] }];
    expect(() =>
      saveCollection(original, { id: "b", name: " WEEKEND ", gameIds: [] }, []),
    ).toThrow("already exists");
    expect(() =>
      saveCollection(original, { id: "b", name: "\n", gameIds: [] }, []),
    ).toThrow();
    expect(() =>
      saveCollection(
        original,
        { id: "b", name: "x".repeat(81), gameIds: [] },
        [],
      ),
    ).toThrow();
    expect(
      saveCollection(original, { id: "a", name: "Weekend", gameIds: [] }, []),
    ).toEqual(original);
  });
});

describe("saved grouping collections", () => {
  it("keeps copied rules live for late additions, changed tags, removals and hidden games", () => {
    const games = demoGames()
      .slice(0, 2)
      .map((g, i) => ({
        ...g,
        genres: [i ? "Action" : "RPG"],
        tags: [],
        hidden: false,
      }));
    const rule = {
      mode: "smart" as const,
      groupId: "smart:rpg",
      groups: [
        {
          id: "rpg",
          name: "RPGs",
          match: "all" as const,
          conditions: [
            { id: "tag", field: "tags" as const, op: "hasAny", value: "RPG" },
          ],
        },
      ],
    };
    const saved = saveCollection(
      [],
      { id: "live", name: "RPGs", gameIds: ["stale"], rule },
      games,
    );
    rule.groups[0].conditions[0].value = "Action";
    expect(saved[0].gameIds).toEqual([]);
    const late = {
      ...structuredClone(games[0]),
      id: "late",
      title: "New game",
      hidden: true,
      notes: "Keep me",
      favorite: true,
    };
    games.push(late);
    let resolved = resolveCollections(saved, games);
    expect(resolved[0].gameIds).toEqual([games[0].id, "late"]);
    expect(
      filterGames(
        games,
        "collection:live",
        "",
        false,
        "",
        "title",
        resolved,
      ).map((g) => g.id),
    ).toEqual([games[0].id]);
    games[0].genres = ["Action"];
    resolved = resolveCollections(saved, games.slice(1));
    expect(resolved[0].gameIds).toEqual(["late"]);
    expect(late).toMatchObject({ notes: "Keep me", favorite: true });
    expect(setMembership(saved, "live", games[1].id, true)).toEqual(saved);
  });
  it("copies priority and relative rules independently and can freeze current matches", () => {
    const now = Date.now(),
      g = { ...demoGames()[0], addedAt: now };
    const rule = {
      mode: "smart" as const,
      groupId: "smart:recent",
      groups: [
        {
          id: "recent",
          name: "Recent",
          match: "all" as const,
          conditions: [
            { id: "date", field: "added" as const, op: "within", value: "7" },
          ],
        },
      ],
    };
    const live = saveCollection(
      [],
      { id: "c", name: "Recent", gameIds: [], rule },
      [g],
    );
    const snapshot = saveCollection(
      [],
      {
        id: "s",
        name: "Snapshot",
        gameIds: resolveCollections(live, [g], now)[0].gameIds,
      },
      [g],
    );
    expect(
      resolveCollections(live, [g], now + 8 * 86400000)[0].gameIds,
    ).toEqual([]);
    expect(
      resolveCollections(snapshot, [g], now + 8 * 86400000)[0].gameIds,
    ).toEqual([g.id]);
    expect(
      resolveCollections(JSON.parse(JSON.stringify(live)), [g], now)[0].gameIds,
    ).toEqual([g.id]);
  });
  it("saves built-in groups and rejects a missing smart group", () => {
    const g = demoGames()[0];
    const saved = saveCollection(
      [],
      {
        id: "tag",
        name: "Tag",
        gameIds: [],
        rule: {
          mode: "tags",
          groupId: "tag:" + g.genres[0].toLowerCase(),
          groups: [],
        },
      },
      [g],
    );
    expect(resolveCollections(saved, [g])[0].gameIds).toEqual([g.id]);
    expect(() =>
      saveCollection(
        [],
        {
          id: "bad",
          name: "Bad",
          gameIds: [],
          rule: { mode: "smart", groupId: "smart:gone", groups: [] },
        },
        [g],
      ),
    ).toThrow();
  });
});

it("updates HLTB collections when current metadata arrives or identity is rematched", () => {
  const g = demoGames()[0];
  g.enrichment = undefined;
  const [saved] = saveCollection(
    [],
    {
      id: "short",
      name: "Short",
      gameIds: [],
      rule: { mode: "hltb", groupId: "hltb:0", groups: [] },
    },
    [g],
  );
  expect(collectionMembers(saved, [g])).toEqual([]);
  g.enrichment = {
    hltb: {
      status: "ready",
      fingerprint: identity(g, "hltb"),
      checkedAt: 1,
      nextCheckAt: 2,
      data: { extraHours: 5 },
    },
  };
  expect(collectionMembers(saved, [g])).toEqual([g.id]);
  g.title = "Rematched identity";
  expect(collectionMembers(saved, [g])).toEqual([]);
  expect(collectionMembers(saved, [])).toEqual([]);
});
