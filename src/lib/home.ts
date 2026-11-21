import { recentSortAt } from "./play-activity";
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
type RankedGame = { game: Game; rank: number };
const descendingRank = (a: RankedGame, b: RankedGame) =>
  b.rank - a.rank || byTitle(a.game, b.game);
const ascendingRank = (a: RankedGame, b: RankedGame) =>
  a.rank - b.rank || byTitle(a.game, b.game);

export function createHomeShelfBuilder() {
  // Identity validation is reusable for an immutable game snapshot.
  const hoursByGame = new WeakMap<Game, number>();
  return (
    games: Game[],
    collections: GameCollection[],
    now = Date.now(),
    collectionGames?: Map<string, Game[]>,
  ) =>
    buildHomeShelves(games, collections, now, collectionGames, (game) => {
      const cached = hoursByGame.get(game);
      if (cached !== undefined) return cached;
      const hours = mainExtraHours(game) ?? 0;
      hoursByGame.set(game, hours);
      return hours;
    });
}

export function homeCollectionMembers(games: Game[], collections: GameCollection[]) {
  const lists = new Map<string, Game[]>();
  const memberships = new Map<string, Game[][]>();
  for (const collection of collections) {
    const list: Game[] = [];
    lists.set(collection.id, list);
    for (const id of new Set(collection.gameIds)) {
      const targets = memberships.get(id);
      if (targets) targets.push(list);
      else memberships.set(id, [list]);
    }
  }
  if (memberships.size) {
    for (const game of games) {
      const targets = memberships.get(game.id);
      if (targets) for (const list of targets) list.push(game);
    }
  }
  return lists;
}
// Three played entries per addition keep activity focused on returning to games.
export function recentActivity(played: Game[], added: Game[]) {
  // Reserve every played ID before mixing so even plays later in the stream
  // cannot occupy an addition slot.
  const seen = new Set<string>();
  const uniquePlayed: Game[] = [];
  for (const game of played) {
    if (seen.has(game.id)) continue;
    seen.add(game.id);
    uniquePlayed.push(game);
  }
  const result: Game[] = [];
  let p = 0;
  let a = 0;
  while (p < uniquePlayed.length || a < added.length) {
    for (let i = 0; i < 3 && p < uniquePlayed.length; i++)
      result.push(uniquePlayed[p++]);
    while (a < added.length) {
      const game = added[a++];
      if (seen.has(game.id)) continue;
      seen.add(game.id);
      result.push(game);
      break;
    }
  }
  return result;
}
export function buildHomeShelves(
  games: Game[],
  collections: GameCollection[],
  now = Date.now(),
  collectionGames?: Map<string, Game[]>,
  readHours = mainExtraHours,
): HomeShelf[] {
  const plays: RankedGame[] = [],
    releases: RankedGame[] = [],
    shortTimes: RankedGame[] = [],
    longTimes: RankedGame[] = [];
  for (const g of games) {
    const playedAt = recentSortAt(g);
    if (playedAt && playedAt <= now) plays.push({ game: g, rank: playedAt });
    const release = releaseSpan(g)?.[0];
    if (release && release > 0 && release <= now)
      releases.push({ game: g, rank: release });
    const hours = g.enrichment?.hltb?.data?.extraHours ? readHours(g) : undefined;
    if (hours && hours > 0 && hours <= 10)
      shortTimes.push({ game: g, rank: hours });
    else if (hours !== undefined && hours >= 30)
      longTimes.push({ game: g, rank: hours });
  }
  const played = plays.sort(descendingRank).map(({ game }) => game);
  const added = [...games].sort(
    (a, b) => b.addedAt - a.addedAt || byTitle(a, b),
  );
  const released = releases.sort(descendingRank).map(({ game }) => game);
  const short = shortTimes.sort(ascendingRank).map(({ game }) => game);
  const long = longTimes.sort(descendingRank).map(({ game }) => game);
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
      description: "Latest Steam and SlopLauncher activity",
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
    ...collections.map((c) => {
      let list = collectionGames?.get(c.id);
      if (!list) {
        const ids = new Set(c.gameIds);
        list = games.filter((g) => ids.has(g.id));
      }
      return {
        id: `collection:${c.id}`,
        title: c.name,
        description: c.rule ? "Automatic collection" : "Your collection",
        games: list,
      };
    }),
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
