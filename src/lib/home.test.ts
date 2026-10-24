import { it, expect } from "vitest";
import {
  buildHomeShelves,
  recentActivity,
  orderHomeShelves,
  moveHomeRow,
} from "./home";
import { demoGames } from "./demo";
import { identity } from "./enrichment-identity";
import type { Game } from "./types";
const now = Date.UTC(2026, 9, 24);
function game(
  id: string,
  played = 0,
  added = 1,
  hours?: number,
  release?: number,
): Game {
  const g = {
    ...demoGames()[0],
    id,
    title: id,
    lastPlayed: played || undefined,
    addedAt: added,
    releasedAt: release,
    year: undefined,
    enrichment: undefined,
  } as Game;
  if (hours !== undefined)
    g.enrichment = {
      hltb: {
        status: "ready",
        checkedAt: now,
        nextCheckAt: now + 10000,
        fingerprint: identity(g, "hltb"),
        data: { extraHours: hours },
      },
    };
  return g;
}
it("mixes recent plays ahead of additions without repeating games", () => {
  const played = Array.from({ length: 6 }, (_, i) => game(`played-${i}`));
  const added = [played[0], game("new-1"), game("new-2"), ...played];
  expect(recentActivity(played, added).map((g) => g.id)).toEqual([
    "played-0",
    "played-1",
    "played-2",
    "new-1",
    "played-3",
    "played-4",
    "played-5",
    "new-2",
  ]);
});
it("builds the default shelf order from current times, releases and live collection membership", () => {
  const short = game("short", now - 100, 1, 5, Date.UTC(2026, 0, 1)),
    long = game("long", now - 500, 2, 60, Date.UTC(2024, 0, 1)),
    unknown = game("unknown", 0, 3, undefined, Date.UTC(2027, 0, 1));
  const shelves = buildHomeShelves(
    [short, long, unknown],
    [{ id: "c", name: "Collection", gameIds: [long.id] }],
    now,
  );
  expect(shelves.map((s) => s.id)).toEqual([
    "activity",
    "played",
    "added",
    "released",
    "short",
    "long",
    "collection:c",
  ]);
  expect(
    shelves.find((s) => s.id === "played")!.games.map((g) => g.id),
  ).toEqual(["short", "long"]);
  expect(shelves.find((s) => s.id === "added")!.games[0].id).toBe("unknown");
  expect(
    shelves.find((s) => s.id === "released")!.games.map((g) => g.id),
  ).toEqual(["short", "long"]);
  expect(shelves.find((s) => s.id === "short")!.games.map((g) => g.id)).toEqual(
    ["short"],
  );
  expect(shelves.find((s) => s.id === "long")!.games.map((g) => g.id)).toEqual([
    "long",
  ]);
  const late = game("late", now, 4, 2);
  const changed = buildHomeShelves(
    [long, late],
    [{ id: "new", name: "New collection", gameIds: [late.id] }],
    now,
  );
  expect(changed.find((s) => s.id === "short")!.games.map((g) => g.id)).toEqual(
    ["late"],
  );
  expect(changed.find((s) => s.id === "played")!.games[0].id).toBe("late");
  expect(changed.at(-1)!.games[0].id).toBe("late");
  const rematched = { ...late, title: "Rematched" };
  expect(
    buildHomeShelves([rematched], [], now).find((s) => s.id === "short")!.games,
  ).toEqual([]);
});
it("keeps saved ordering through additions and deletions, and moves rows before or after targets", () => {
  const shelves = buildHomeShelves(
    [],
    [{ id: "late", name: "Late", gameIds: [] }],
    now,
  );
  const ordered = orderHomeShelves(shelves, [
    "long",
    "removed",
    "activity",
    "long",
  ]);
  expect(ordered.map((s) => s.id)).toEqual([
    "long",
    "activity",
    "played",
    "added",
    "released",
    "short",
    "collection:late",
  ]);
  expect(moveHomeRow(["a", "b", "c"], "c", "a")).toEqual(["c", "a", "b"]);
  expect(moveHomeRow(["a", "b", "c"], "a", "c", true)).toEqual(["b", "c", "a"]);
  const restored = JSON.parse(JSON.stringify(ordered.map((s) => s.id)));
  expect(orderHomeShelves(shelves, restored).map((s) => s.id)).toEqual(
    restored,
  );
});

it("keeps the activity shelf mixed when the latest additions were already played", () => {
  const played = Array.from({ length: 8 }, (_, i) => game(`played-${i}`));
  const fresh = game("new-unplayed");
  expect(
    recentActivity(played, [...played, fresh])
      .slice(0, 4)
      .map((g) => g.id),
  ).toEqual(["played-0", "played-1", "played-2", "new-unplayed"]);
});
