import type { Game, GameCollection } from "./types";
import { mainExtraHours, releaseSpan } from "./game-facts";
export interface HomeShelf {
  id: string;
  title: string;
  description: string;
  games: Game[];
}
export const HOME_ROW_DRAG = "application/x-sloplauncher-home-row";
const byTitle = (a: Game, b: Game) => a.title.localeCompare(b.title);
// Three played entries per addition keep activity focused on returning to games.
export function recentActivity(played: Game[], added: Game[]) {
  // A game already represented in the played stream must not consume an
  // addition slot, or the mixed shelf can accidentally mirror Recently Played.
  const playedIds = new Set(played.map((g) => g.id));
  added = added.filter((g) => !playedIds.has(g.id));
  const result: Game[] = [],
    seen = new Set<string>();
  let p = 0,
    a = 0;
  const take = (list: Game[], index: number) => {
    const game = list[index];
    if (game && !seen.has(game.id)) {
      seen.add(game.id);
      result.push(game);
    }
  };
  while (p < played.length || a < added.length) {
    for (let i = 0; i < 3 && p < played.length;) {
      const game = played[p++];
      if (!seen.has(game.id)) {
        take([game], 0);
        i++;
      }
    }
    while (a < added.length && seen.has(added[a].id)) a++;
    if (a < added.length) take(added, a++);
  }
  return result;
}
export function buildHomeShelves(
  games: Game[],
  collections: GameCollection[],
  now = Date.now(),
  collectionGames?: Map<string, Game[]>,
): HomeShelf[] {
  const played = games
    .filter((g) => g.lastPlayed && g.lastPlayed <= now)
    .sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || byTitle(a, b));
  const added = [...games].sort(
    (a, b) => b.addedAt - a.addedAt || byTitle(a, b),
  );
  const dates = new Map(games.map((g) => [g.id, releaseSpan(g)?.[0]]));
  const released = games
    .filter((g) => dates.get(g.id)! > 0 && dates.get(g.id)! <= now)
    .sort((a, b) => dates.get(b.id)! - dates.get(a.id)! || byTitle(a, b));
  const hours = new Map(games.map((g) => [g.id, mainExtraHours(g)]));
  const short = games
    .filter((g) => hours.get(g.id)! > 0 && hours.get(g.id)! <= 10)
    .sort((a, b) => hours.get(a.id)! - hours.get(b.id)! || byTitle(a, b));
  const long = games
    .filter((g) => hours.get(g.id)! >= 30)
    .sort((a, b) => hours.get(b.id)! - hours.get(a.id)! || byTitle(a, b));
  return [
    {
      id: "activity",
      title: "Recent Activity",
      description: "Recently played, mixed with new additions",
      games: recentActivity(played, added),
    },
    {
      id: "played",
      title: "Recently Played",
      description: "Pick up where you left off",
      games: played,
    },
    {
      id: "added",
      title: "Recently Added",
      description: "The newest games in your library",
      games: added,
    },
    {
      id: "released",
      title: "Recently Released",
      description: "Latest releases in your library",
      games: released,
    },
    {
      id: "short",
      title: "HLTB Short Games",
      description: "Main + Extras · 10 hours or less",
      games: short,
    },
    {
      id: "long",
      title: "HLTB Long Games",
      description: "Main + Extras · 30 hours or more",
      games: long,
    },
    ...collections.map((c) => ({
      id: `collection:${c.id}`,
      title: c.name,
      description: c.rule ? "Automatic collection" : "Your collection",
      games:
        collectionGames?.get(c.id) ??
        games.filter((g) => c.gameIds.includes(g.id)),
    })),
  ];
}
// Reconcile every render: newly created collections appear without a migration,
// removed rows disappear, and surviving rows keep the user's saved order.
export function orderHomeShelves(shelves: HomeShelf[], saved: string[] = []) {
  const byId = new Map(shelves.map((s) => [s.id, s]));
  const ids = [...new Set([...saved, ...shelves.map((s) => s.id)])];
  return ids.map((id) => byId.get(id)).filter((s): s is HomeShelf => !!s);
}
export function moveHomeRow(
  ids: string[],
  moving: string,
  target: string,
  after = false,
) {
  if (moving === target || !ids.includes(moving) || !ids.includes(target))
    return ids;
  const next = ids.filter((id) => id !== moving);
  next.splice(next.indexOf(target) + (after ? 1 : 0), 0, moving);
  return next;
}
