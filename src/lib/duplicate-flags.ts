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
    games: games.map((game) => {
      const patch = patches.get(game.id);
      return patch ? { ...game, ...patch } : game;
    }),
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
  return changeDuplicateFlags(games, index, [id], flag, value, now);
}

export function changeDuplicateFlags(
  games: Game[],
  index: Index,
  ids: readonly string[],
  flag: Flag,
  value: boolean,
  now = Date.now(),
) {
  const key = timestamp(flag);
  const visited = new Set<string>();
  const updates: GameFlagsUpdate[] = [];
  for (const id of ids) {
    if (visited.has(id)) continue;
    const members = index.members(id);
    let at = now;
    for (const member of members)
      at = Math.max(at, (member[key] ?? 0) + 1);
    for (const member of members) {
      visited.add(member.id);
      updates.push({ id: member.id, [flag]: value, [key]: at });
    }
  }
  return apply(games, updates);
}

/** Runs on every library/identity change, not just initial discovery. */
export function reconcileDuplicateFlags(games: Game[], index: Index) {
  const visited = new Set<string>();
  const updates: GameFlagsUpdate[] = [];
  for (const game of games) {
    if (visited.has(game.id)) continue;
    const members = index.members(game.id);
    if (members.length < 2) continue;
    for (const member of members) visited.add(member.id);
    const values = fields.map((flag) => {
      const key = timestamp(flag);
      let at = 0;
      let value = false;
      for (const member of members) {
        const memberAt = member[key] ?? 0;
        if (memberAt > at) {
          at = memberAt;
          value = !!member[flag];
        } else if (memberAt === at && member[flag]) {
          // Retain positive preferences when timestamps tie or are absent.
          value = true;
        }
      }
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
