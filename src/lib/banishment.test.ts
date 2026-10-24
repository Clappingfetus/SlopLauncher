import { it, expect } from "vitest";
import { demoGames } from "./demo";
import {
  allowedGames,
  blacklistKeys,
  recoveryGame,
  expandBlacklistTargets,
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
