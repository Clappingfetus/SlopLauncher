import { it, expect } from "vitest";
import { demoGames } from "./demo";
import {
  allowedGames,
  blacklistMatches,
  blacklistKeys,
  recoveryGame,
  expandBlacklistTargets,
  groupedBanished,
  type BanishedGame,
} from "./banishment";
import { filterGames } from "./library";
it("blacklists late additions and renamed store copies across libraries while distinguishing verified games", () => {
  const game = {
    ...demoGames()[0],
    id: "a",
    title: "Tomb Raider",
    metadataId: 100,
    notes: "keep",
    favorite: true,
    cover: "https://example.test/art",
    copies: [{ ...demoGames()[0].copies[0], sourceId: "100" }],
  };
  const entry: BanishedGame = {
    id: game.id,
    title: game.title,
    banishedAt: 1,
    keys: blacklistKeys(game),
    game: recoveryGame(game),
    artworkHashes: [],
  };
  const renamed = { ...game, id: "renamed", title: "Another name" };
  const crossStore = {
    ...game,
    id: "new",
    title: "TOMB—RAIDER",
    copies: [
      {
        id: "newcopy",
        source: "gog" as const,
        sourceId: "123",
        installed: false,
      },
    ],
  };
  const distinct = {
    ...crossStore,
    id: "different",
    metadataId: 999,
    copies: [
      { ...crossStore.copies[0], id: "different-copy", sourceId: "999" },
    ],
  };
  expect(
    allowedGames([renamed, crossStore, distinct], [entry]).map((g) => g.id),
  ).toEqual(["different"]);
  expect(
    expandBlacklistTargets([game, crossStore, distinct], [game.id]),
  ).toEqual(new Set([game.id, crossStore.id]));
  expect(entry.game.cover).toBeUndefined();
  expect(entry.game.enrichment).toBeUndefined();
  expect(entry.game.notes).toBe("keep");
  expect(entry.game.favorite).toBe(true);
});
it("rebuilds banished groups and title matches from current identities", () => {
  const makeGame = (id: string, metadataId?: number) => ({
    ...demoGames()[0],
    id,
    title: "Same title",
    metadataId,
    copies: [{ id, source: "gog" as const, sourceId: id, installed: false }],
  });
  const entry = (game: ReturnType<typeof makeGame>): BanishedGame => ({
    id: game.id,
    title: game.title,
    keys: blacklistKeys(game),
    game: recoveryGame(game),
    banishedAt: 1,
    artworkHashes: [],
  });
  const first = entry(makeGame("first", 100));
  const second = entry(makeGame("second", 200));
  const bridge = entry(makeGame("bridge"));
  const retained = makeGame("retained", 300);
  const rematched = { ...retained, metadataId: 100, title: "New title" };

  expect(groupedBanished([first, second])).toEqual([first, second]);
  expect(groupedBanished([first, second, bridge])).toEqual([first]);
  expect(allowedGames([retained, rematched], [first, second])).toEqual([retained]);
  expect(allowedGames([retained], [first, second, bridge])).toEqual([]);
  expect(allowedGames([rematched], [second, bridge])).toEqual([rematched]);
});
it("filters current store copies and matches even when a blocked title is missing", () => {
  const makeGame = (id: string, sourceId: string, metadataId?: number) => ({
    ...demoGames()[0],
    id,
    title: "Café Racer",
    metadataId,
    copies: [{ id, source: "steam" as const, sourceId, installed: false }],
  });
  const original = makeGame("original", "00041");
  const entry = (game: ReturnType<typeof makeGame>, keys = blacklistKeys(game)) => ({
    id: game.id,
    title: game.title,
    keys,
    game: recoveryGame(game),
    banishedAt: 1,
    artworkHashes: [],
  });
  const blocked = entry(original, ["match:steam:41", "copy:steam:00041"]);
  const matchedByTitle = entry(makeGame("other", "61"));
  const late = { ...makeGame("late", "launcher-id"), title: "Café—Racer" };
  const distinct = makeGame("distinct", "63", 41);
  const rematched = { ...late, title: "New title", metadataId: 41 };
  const renamedCopy = { ...original, id: "new-id", title: "New title" };

  for (const games of [[late, distinct], [rematched, distinct, renamedCopy], [distinct]]) {
    for (const entries of [[blocked], [matchedByTitle], [blocked, matchedByTitle], []]) {
      expect(allowedGames(games, entries)).toEqual(
        games.filter((game) => !entries.some(({ keys }) => blacklistMatches(game, keys))),
      );
    }
  }
  expect(allowedGames([late, distinct], [blocked])).toEqual([late, distinct]);
  expect(allowedGames([late, distinct], [blocked, matchedByTitle])).toEqual([distinct]);
  expect(allowedGames([rematched, distinct, renamedCopy], [blocked])).toEqual([distinct]);
});
it("combines a source filter with favorites, tags and collection filters", () => {
  const games = demoGames();
  const game = {
    ...games[0],
    favorite: true,
    tags: ["Co-op"],
    copies: [
      { id: "copy", source: "gog" as const, sourceId: "1", installed: true },
    ],
  };
  const library = [
    game,
    ...games.slice(1).map((g) => ({ ...g, favorite: false })),
  ];
  expect(
    filterGames(
      library,
      "favorites",
      "",
      true,
      "co-op",
      "title",
      [],
      false,
      "gog",
    ).map((g) => g.id),
  ).toEqual([game.id]);
  expect(
    filterGames(
      library,
      "favorites",
      "",
      true,
      "co-op",
      "title",
      [],
      false,
      "steam",
    ),
  ).toEqual([]);
  const collections = [{ id: "c", name: "Collection", gameIds: [game.id] }];
  expect(
    filterGames(
      library,
      "collection:c",
      "",
      false,
      "",
      "title",
      collections,
      false,
      "gog",
    ),
  ).toHaveLength(1);
});
