import { describe, it, expect } from "vitest";
import {
  filterGames,
  mergeDiscovered,
  mergeOwnedLibrary,
  preferredCopy,
} from "./library";
import { demoGames } from "./demo";
describe("library queries", () => {
  it("combines source, installation and every search word", () => {
    const games = demoGames();
    expect(
      filterGames(games, "steam", "ASTER vale", true, "", "title").map(
        (g) => g.title,
      ),
    ).toEqual(["Aster Vale"]);
    expect(filterGames(games, "gog", "ASTER", true, "", "title")).toEqual([]);
  });
  it("favorites and recent have distinct semantics", () => {
    const games = demoGames();
    expect(
      filterGames(games, "favorites", "", false, "", "title"),
    ).toHaveLength(3);
    expect(filterGames(games, "recent", "", false, "", "recent")).toHaveLength(
      5,
    );
  });
  it("puts unknown release dates after known dates", () => {
    const games = demoGames();
    games[0].year = undefined;
    expect(filterGames(games, "all", "", false, "", "year").at(-1)?.id).toBe(
      games[0].id,
    );
  });
});
describe("non-destructive discovery", () => {
  it("retains unchanged rows and accepts late discoveries in large repeated scans", () => {
    const base = demoGames()[0];
    const games = Array.from({ length: 10000 }, (_, i) => ({
      ...base,
      id: `scan-${i}`,
      copies: [{ ...base.copies[0], id: `steam:${i}`, sourceId: String(i) }],
    }));
    expect(mergeDiscovered(games, structuredClone(games))).toBe(games);
    const late = {
      ...base,
      id: "late-game",
      copies: [{ ...base.copies[0], id: "late-copy" }],
    };
    const result = mergeDiscovered(games, [late, late]);
    expect(result).toHaveLength(10001);
    expect(result[0]).toBe(games[0]);
    expect(result.at(-1)).toBe(late);
  });
  it("keeps locked blank metadata and untouched rows during owned-library sync", () => {
    const game = {
      ...demoGames()[0],
      description: undefined,
      metadataLocked: true,
    };
    const source = game.copies[0].source as "steam";
    game.copies = [
      { ...game.copies[0], ownership: { accountId: "owner", verifiedAt: 1 } },
    ];
    const incoming = {
      ...structuredClone(game),
      description: "Unwanted store description",
      notes: "discard",
    };
    const games = [game];
    expect(mergeOwnedLibrary(games, [incoming], source)).toBe(games);
    const updated = structuredClone(incoming);
    updated.copies[0].ownership!.verifiedAt = 2;
    const merged = mergeOwnedLibrary(games, [updated], source);
    expect(merged[0].description).toBeUndefined();
    expect(merged[0].notes).toBe(game.notes);
    expect(merged[0].copies[0].ownership!.verifiedAt).toBe(2);
    expect(game.copies[0].ownership!.verifiedAt).toBe(1);
  });
  it("merges late folder discoveries without duplicate games or lost launch settings and ownership", () => {
    const current = demoGames()[0];
    current.notes = "Keep my notes";
    current.copies[0].executable = "D:\\Games\\custom.exe";
    current.copies[0].ownership = { accountId: "owner", verifiedAt: 1 };
    const scan = structuredClone(current);
    scan.copies[0].executable = undefined;
    scan.copies[0].ownership = undefined;
    const late = {
      ...structuredClone(current),
      id: "late",
      title: "Added in a second folder",
      copies: [{ ...current.copies[0], id: "steam:999", sourceId: "999" }],
    };
    const merged = mergeDiscovered([current], [scan, scan, late]);
    expect(merged).toHaveLength(2);
    expect(merged[0].notes).toBe("Keep my notes");
    expect(merged[0].copies[0].executable).toBe("D:\\Games\\custom.exe");
    expect(merged[0].copies[0].ownership?.accountId).toBe("owner");
  });
  it("preserves edited metadata, notes and favorite on repeated imports", () => {
    const g = demoGames()[0];
    const old = {
      ...g,
      title: "My edited title",
      notes: "Remember this",
      favorite: true,
    };
    const incoming = {
      ...g,
      title: "Store title",
      notes: "",
      favorite: false,
      copies: [{ ...g.copies[0], installed: false }],
    };
    const result = mergeDiscovered([old], [incoming]);
    expect(result).toHaveLength(1);
    expect(result[0]).toMatchObject({
      title: "My edited title",
      notes: "Remember this",
      favorite: true,
    });
    expect(result[0].copies[0].installed).toBe(false);
    expect(old.copies[0].installed).toBe(true);
  });
  it("does not merge unrelated editions with similar names", () => {
    const a = demoGames()[0];
    const b = {
      ...a,
      id: "other",
      copies: [{ ...a.copies[0], id: "other-copy" }],
    };
    expect(mergeDiscovered([a], [b])).toHaveLength(2);
  });
  it("never deletes games missing from a failed or empty scan", () => {
    expect(mergeDiscovered(demoGames(), [])).toHaveLength(12);
  });
  it("uses explicit preferred copy before automatic installed selection", () => {
    const game = demoGames()[0];
    game.copies.push({ ...game.copies[0], id: "gog:123", source: "gog" });
    game.preferredCopy = "gog:123";
    expect(preferredCopy(game).source).toBe("gog");
  });
});
it("keeps the Hidden Games view independent of global visibility and supports late changes", () => {
  const games = demoGames();
  games[0].hidden = true;
  for (const show of [false, true]) {
    expect(filterGames(games, "hidden", "", false, "", "title", [], show).map(g => g.id)).toEqual([games[0].id]);
  }
  const late = {...games[1], id: "late-hidden", hidden: true};
  expect(filterGames([...games, late], "hidden", "", false, "", "title")).toHaveLength(2);
  games[0].hidden = false;
  expect(filterGames(games, "hidden", "", false, "", "title")).toEqual([]);
  expect(filterGames([late], "hidden", "not-a-match", false, "", "title")).toEqual([]);
});
