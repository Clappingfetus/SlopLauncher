import { describe, it, expect } from "vitest";
import { filterGames, mergeDiscovered, mergeOwnedLibrary } from "./library";
import { collectionCounts, setMembership } from "./collections";
import { identity, presentGame } from "./enrichment";
import { artworkIdentity } from "./artwork";
import { demoGames } from "./demo";
import type { Game, GameCollection } from "./types";
describe("hidden game state", () => {
  it("excludes hidden games from every view and includes them when requested", () => {
    const game = {
      ...demoGames()[0],
      hidden: true,
      favorite: true,
      lastPlayed: 12,
    };
    const folder: GameCollection = {
      id: "folder",
      name: "Keep",
      gameIds: [game.id],
    };
    for (const filter of [
      "all",
      "favorites",
      "recent",
      "collection:folder",
      game.copies[0].source,
    ] as const) {
      expect(
        filterGames([game], filter, "", false, "", "title", [folder]),
      ).toEqual([]);
      expect(
        filterGames([game], filter, "", false, "", "title", [folder], true),
      ).toEqual([game]);
    }
    expect(
      filterGames(
        [game],
        "all",
        game.title,
        true,
        game.genres[0],
        "title",
        [],
        true,
      ),
    ).toEqual([game]);
    expect(
      filterGames([game], "all", "never matches", false, "", "title", [], true),
    ).toEqual([]);
  });
  it("preserves hidden/favorite state across rediscovery, account imports and rematches", () => {
    const old: Game = {
      ...demoGames()[0],
      hidden: true,
      favorite: true,
      notes: "Keep me",
    };
    const incoming: Game = {
      ...structuredClone(old),
      hidden: false,
      favorite: false,
      notes: "",
      copies: old.copies.map((c) => ({
        ...c,
        ownership: { accountId: "test", verifiedAt: 2 },
      })),
    };
    for (const merged of [
      mergeDiscovered([old], [incoming]),
      mergeOwnedLibrary([old], [incoming], old.copies[0].source as "steam"),
    ]) {
      expect(merged[0]).toMatchObject({
        hidden: true,
        favorite: true,
        notes: "Keep me",
      });
      const rematched = {
        ...merged[0],
        title: "A corrected title",
        metadataId: 42,
      };
      expect(presentGame(rematched, 1).hidden).toBe(true);
    }
    const visible = { ...old, hidden: false };
    expect(identity(old, "steamgriddb", 1)).toBe(
      identity(visible, "steamgriddb", 1),
    );
    expect(artworkIdentity(old)).toBe(artworkIdentity(visible));
  });
  it("keeps memberships and favorites when a game is hidden, and handles future games independently", () => {
    const original = { ...demoGames()[0], hidden: true, favorite: true };
    const late = {
      ...structuredClone(original),
      id: "future",
      hidden: undefined,
      copies: [{ ...original.copies[0], id: "future-copy" }],
    };
    const folders = setMembership(
      [{ id: "weekend", name: "Weekend", gameIds: [original.id] }],
      "weekend",
      late.id,
      true,
    );
    const games = mergeDiscovered([original], [late]);
    expect(
      filterGames(
        games,
        "collection:weekend",
        "",
        false,
        "",
        "title",
        folders,
      ).map((g) => g.id),
    ).toEqual(["future"]);
    expect(folders[0].gameIds).toEqual([original.id, late.id]);
    expect(games[0].favorite).toBe(true);
    expect(collectionCounts(folders, games).get("weekend")).toBe(2);
    expect(
      filterGames(
        games,
        "collection:weekend",
        "",
        false,
        "",
        "title",
        folders,
        true,
      ),
    ).toHaveLength(2);
    expect(
      filterGames(
        games.filter((g) => g.id !== late.id),
        "all",
        "",
        false,
        "",
        "title",
      ),
    ).toEqual([]);
    expect(
      filterGames(demoGames(), "all", "", false, "", "title"),
    ).toHaveLength(12);
  });
});
