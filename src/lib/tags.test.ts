import { describe, it, expect } from "vitest";
import { mergeTags, tagKey, libraryTags } from "./tags";
import { gameTags, identity, presentGame } from "./enrichment";
import { filterGames, mergeOwnedLibrary, mergeDiscovered } from "./library";
import { demoGames } from "./demo";
import type { Game, ProviderId } from "./types";

function game(): Game {
  return {
    ...demoGames()[0],
    tags: ["My backlog"],
    genres: ["RPG"],
    enrichment: undefined,
  };
}
function imported(
  g: Game,
  provider: ProviderId,
  tags: string[],
  genres: string[] = [],
) {
  g.enrichment = {
    ...g.enrichment,
    [provider]: {
      status: "ready",
      fingerprint: identity(g, provider),
      checkedAt: 1,
      nextCheckAt: 1000,
      data: { tags, genres },
    },
  };
}
describe("game tags", () => {
  it("combines genres, custom tags, Steam and metadata taxonomies without duplicates", () => {
    const g = game();
    imported(g, "steam", ["rpg", "Open World"], ["Action"]);
    imported(g, "igdb", ["Fantasy", "Open World"], ["Role-playing"]);
    expect(gameTags(g)).toEqual([
      "Action",
      "Fantasy",
      "My backlog",
      "Open World",
      "Role-playing",
      "RPG",
    ]);
    expect(presentGame(g).tags).toEqual(gameTags(g));
    expect(g.tags).toEqual(["My backlog"]);
  });
  it("normalizes case, Unicode and whitespace but preserves distinct tags", () => {
    expect(
      mergeTags([
        " RPG ",
        "rpg",
        "ＲＰＧ",
        " Open  World ",
        "Psychological Horror",
        "Horror",
      ]),
    ).toEqual(["Horror", "Open World", "Psychological Horror", "RPG"]);
    expect(tagKey("Open  World")).toBe("open world");
    expect(mergeTags(["", "x".repeat(81)])).toEqual([]);
  });
  it("does not keep an old provider's tags after rename, rematch or source identity changes", () => {
    const g = game();
    imported(g, "steam", ["Old tag"]);
    imported(g, "igdb", ["Old genre"]);
    const renamed = { ...g, title: "A different game" };
    expect(gameTags(renamed)).toEqual(["My backlog", "RPG"]);
    expect(gameTags({ ...g, metadataId: 987654 })).not.toContain("Old tag");
    expect(
      gameTags({ ...g, copies: [{ ...g.copies[0], sourceId: "different" }] }),
    ).not.toContain("Old genre");
    imported(renamed, "steam", ["New tag"]);
    expect(gameTags(renamed)).toContain("New tag");
  });
  it("preserves custom tags and base genres when automatic metadata is locked", () => {
    const g = game();
    imported(g, "steam", ["Automatic"]);
    g.metadataLocked = true;
    expect(presentGame(g).tags).toEqual(["My backlog", "RPG"]);
  });
  it("filters exact tags and searches titles, custom tags and genres", () => {
    const g = game(),
      other = {
        ...game(),
        id: "other",
        title: "Other",
        genres: [],
        tags: ["Psychological Horror"],
      };
    expect(
      filterGames([g, other], "all", "", false, "Horror", "title"),
    ).toHaveLength(0);
    expect(filterGames([g, other], "all", "", false, " rpg ", "title")).toEqual(
      [g],
    );
    expect(
      filterGames([g, other], "all", "backlog", false, "", "title"),
    ).toEqual([g]);
    expect(
      filterGames([g, other], "all", "aster RPG", false, "", "title"),
    ).toEqual([g]);
  });
  it("keeps custom tags through repeated discovery and account import", () => {
    const g = game(),
      scan = { ...g, tags: [] };
    expect(mergeDiscovered([g], [scan])[0].tags).toEqual(g.tags);
    scan.copies = [
      { ...scan.copies[0], ownership: { accountId: "fixture", verifiedAt: 1 } },
    ];
    expect(mergeOwnedLibrary([g], [scan], "steam")[0].tags).toEqual(g.tags);
  });
  it("updates available tags when future games arrive and keeps more than 256 across a library", () => {
    const games = Array.from({ length: 400 }, (_, i) => ({
      ...game(),
      id: String(i),
      tags: [`Unique ${i}`],
    }));
    expect(libraryTags(games)).toHaveLength(401);
    const future = { ...game(), id: "future", tags: ["Future arrival"] };
    expect(libraryTags([...games, future])).toContain("Future arrival");
  });
});
