import { it, expect } from "vitest";
import { releaseLabel, exactReleaseDate } from "./release-date";
import { releaseSpan } from "./game-facts";
import { buildHomeShelves } from "./home";
import { demoGames } from "./demo";
import { filterGames, mergeOwnedLibrary } from "./library";
import { presentGame, identity } from "./enrichment";
import { missingFields } from "./gaps";
import type { Game } from "./types";
const g = (id: string, at?: number, year?: number): Game => ({
  ...demoGames()[0],
  id,
  title: id,
  year,
  releasedAt: at,
  enrichment: undefined,
});
it("sorts same-year releases by the full calendar date, keeps unknown-year precision, and excludes future days", () => {
  const now = Date.UTC(2026, 9, 24),
    oct = g("A October", Date.UTC(2026, 9, 23), 2026),
    jan = g("Z January", Date.UTC(2026, 0, 3), 2026),
    year = g("Year only", undefined, 2026),
    future = g("Future", Date.UTC(2026, 11, 1), 2026);
  const games = [jan, year, future, oct];
  const shelf = buildHomeShelves(games, [], now).find(
    (s) => s.id === "released",
  )!;
  expect(shelf.games.map((g) => g.id)).toEqual([
    "A October",
    "Z January",
    "Year only",
  ]);
  expect(
    filterGames(games, "all", "", false, "", "year").map((g) => g.id),
  ).toEqual(["Future", "A October", "Z January", "Year only"]);
  expect(releaseSpan(year)).toEqual([
    Date.UTC(2026, 0, 1),
    Date.UTC(2027, 0, 1) - 1,
  ]);
  expect(releaseLabel(year)).toBe("2026");
});
it("renders a calendar date in UTC and rejects invalid persisted times", () => {
  const game = g("Date", Date.UTC(2026, 0, 1), 2026);
  expect(releaseLabel(game)).toContain("Jan 1, 2026");
  expect(exactReleaseDate(NaN)).toBeUndefined();
  expect(exactReleaseDate(Infinity)).toBeUndefined();
  expect(exactReleaseDate(1.2)).toBeUndefined();
  expect(exactReleaseDate(Date.UTC(2300, 0, 1))).toBeUndefined();
});
it("prefers the store's full platform date and preserves locked metadata", () => {
  const game = g("PC port", undefined, 2020);
  game.copies = [
    { id: "s", source: "steam", sourceId: "42", installed: false },
  ];
  game.enrichment = {
    steam: {
      status: "ready",
      checkedAt: 1,
      nextCheckAt: 2,
      fingerprint: identity(game, "steam"),
      data: { year: 2024, releasedAt: Date.UTC(2024, 5, 2) },
    },
    igdb: {
      status: "ready",
      checkedAt: 1,
      nextCheckAt: 2,
      fingerprint: identity(game, "igdb"),
      data: { year: 2020, releasedAt: Date.UTC(2020, 5, 2) },
    },
  };
  expect(presentGame(game).releasedAt).toBe(Date.UTC(2024, 5, 2));
  expect(presentGame(game).year).toBe(2024);
  expect(
    presentGame({ ...game, metadataLocked: true }).releasedAt,
  ).toBeUndefined();
  expect(presentGame({ ...game, metadataLocked: true }).year).toBe(2020);
});
it("imports newly supplied full dates without losing personal data and handles later additions", () => {
  const original = g("gog:42", undefined, 2024);
  original.copies = [
    { id: "gog:42", source: "gog", sourceId: "42", installed: true },
  ];
  original.notes = "Keep";
  original.favorite = true;
  const incoming = {
    ...original,
    copies: original.copies.map(c=>({...c,installed:false,ownership:{accountId:"test",verifiedAt:1}})),
    releasedAt: Date.UTC(2024, 10, 11),
    notes: "",
    favorite: false,
  };
  const merged = mergeOwnedLibrary([original], [incoming], "gog")[0];
  expect(merged.releasedAt).toBe(incoming.releasedAt);
  expect(merged.notes).toBe("Keep");
  expect(merged.favorite).toBe(true);
  const locked = mergeOwnedLibrary(
    [{ ...original, metadataLocked: true }],
    [incoming],
    "gog",
  )[0];
  expect(locked.releasedAt).toBeUndefined();
  const late = g("late", Date.UTC(2025, 4, 2), 2025);
  expect(
    buildHomeShelves([merged, late], [], Date.UTC(2026, 0, 1)).find(
      (s) => s.id === "released",
    )?.games[0].id,
  ).toBe("late");
});
it("counts a year-only release as a manual metadata gap without inventing a day", () => {
  const game = g("Partial", undefined, 2024);
  expect(missingFields(game)).toContain("release date");
  expect(presentGame(game).releasedAt).toBeUndefined();
  expect(
    missingFields({ ...game, releasedAt: Date.UTC(2024, 2, 3) }),
  ).not.toContain("release date");
});
