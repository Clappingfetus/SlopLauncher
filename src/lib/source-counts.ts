import type { Game, Source } from "./types";

// Count each game once per source, even when it has multiple copies from that
// store. Recompute from the current library so imports and removals are visible.
export function countGamesBySource(
  games: readonly Game[],
): Map<Source, number> {
  const counts = new Map<Source, number>();
  for (const game of games) {
    const seen = game.copies.length > 1 ? new Set<Source>() : undefined;
    for (const copy of game.copies) {
      if (seen?.has(copy.source)) continue;
      seen?.add(copy.source);
      counts.set(copy.source, (counts.get(copy.source) ?? 0) + 1);
    }
  }
  return counts;
}
