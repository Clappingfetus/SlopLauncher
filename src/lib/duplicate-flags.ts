import type { Game } from "./types";
import type { GameFlagsUpdate } from "./backend";

type Flag = "favorite" | "hidden";
type Index = { members(id: string): Game[] };
const fields = ["favorite", "hidden"] as const;
const timestamp = (flag: Flag) => `${flag}UpdatedAt` as const;

function apply(games: Game[], updates: GameFlagsUpdate[]) {
  if (!updates.length) return { games, updates };
  const patches = new Map(updates.map((update) => [update.id, update]));
  return {
    games: games.map((game) =>
      patches.has(game.id) ? { ...game, ...patches.get(game.id)! } : game,
    ),
    updates,
  };
}

/** Explicit choices carry a clock so a later import cannot resurrect an old flag. */
export function changeDuplicateFlag(
  games: Game[],
  index: Index,
  id: string,
  flag: Flag,
  value: boolean,
  now = Date.now(),
) {
  const members = index.members(id);
  const key = timestamp(flag);
  const at = Math.max(now, ...members.map((game) => (game[key] ?? 0) + 1));
  return apply(
    games,
    members.map((game) => ({ id: game.id, [flag]: value, [key]: at })),
  );
}

/** Runs on every library/identity change, not just initial discovery. */
export function reconcileDuplicateFlags(games: Game[], index: Index) {
  const visited = new Set<string>();
  const updates: GameFlagsUpdate[] = [];
  for (const game of games) {
    if (visited.has(game.id)) continue;
    const members = index.members(game.id);
    for (const member of members) visited.add(member.id);
    if (members.length < 2) continue;
    const values = fields.map((flag) => {
      const key = timestamp(flag);
      const at = Math.max(...members.map((member) => member[key] ?? 0));
      // Before clocks existed, retain any positive personal preference.
      const value = members.some(
        (member) => (member[key] ?? 0) === at && !!member[flag],
      );
      return { flag, key, at, value };
    });
    for (const member of members) {
      const patch: GameFlagsUpdate = { id: member.id };
      for (const { flag, key, at, value } of values) {
        if (!!member[flag] !== value || (member[key] ?? 0) !== at) {
          patch[flag] = value;
          if (at) patch[key] = at;
        }
      }
      if (Object.keys(patch).length > 1) updates.push(patch);
    }
  }
  return apply(games, updates);
}
