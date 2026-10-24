import type { Game, Library } from "./types";
export interface BanishedGame {
  id: string;
  title: string;
  banishedAt: number;
  keys: string[];
  game: Game;
  artworkHashes: string[];
}
export interface BanishResult {
  library: Library;
  banished: BanishedGame[];
}
export function blacklistKeys(game: Game): string[] {
  const owned = Number(
    game.copies.find((c) => c.source === "steam" && /^\d+$/.test(c.sourceId))
      ?.sourceId,
  );
  const steam =
    Number.isSafeInteger(owned) && owned > 0 ? owned : game.metadataId;
  return [
    `game:${game.id}`,
    `title:${game.title.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "")}`,
    ...game.copies.map((c) => `copy:${c.source}:${c.sourceId}`),
    ...(steam ? [`copy:steam:${steam}`, `match:steam:${steam}`] : []),
  ];
}
export function allowedGames(games: Game[], entries: BanishedGame[]) {
  if (!entries.length) return games;
  return games.filter((g) => !entries.some((b) => blacklistMatches(g, b.keys)));
}
export function blacklistMatches(game: Game, blocked: string[]) {
  const keys = blacklistKeys(game);
  if (keys.some((k) => !k.startsWith("title:") && blocked.includes(k)))
    return true;
  const match = keys.find((k) => k.startsWith("match:steam:")),
    other = blocked.find((k) => k.startsWith("match:steam:"));
  return (
    !(match && other && match !== other) &&
    keys.some((k) => k.startsWith("title:") && blocked.includes(k))
  );
}
export function recoveryGame(game: Game): Game {
  return {
    ...game,
    cover: undefined,
    hero: undefined,
    description: undefined,
    developer: undefined,
    publisher: undefined,
    genres: [],
    tags: [],
    year: undefined,
    releasedAt: undefined,
    screenshots: [],
    storeMetadata: undefined,
    steamActivity: undefined,
    metadataScan: undefined,
    activitySummary: undefined,
    artwork: undefined,
    enrichment: undefined,
    metadataLocked: undefined,
    titleOverride: undefined,
    providerIds: undefined,
    copies: game.copies.map((c) => ({
      ...c,
      storePage: undefined,
      installSizeBytes: undefined,
    })),
  };
}
export function expandBlacklistTargets(games: Game[], ids: string[]) {
  const target = new Set(ids),
    index = new Map<string, Game[]>(),
    byId = new Map(games.map((g) => [g.id, g]));
  for (const game of games)
    for (const key of blacklistKeys(game))
      index.set(key, [...(index.get(key) ?? []), game]);
  const pending = ids.map((id) => byId.get(id)).filter((g): g is Game => !!g);
  while (pending.length) {
    const game = pending.pop()!,
      keys = blacklistKeys(game);
    for (const key of keys)
      for (const next of index.get(key) ?? [])
        if (!target.has(next.id) && blacklistMatches(next, keys)) {
          target.add(next.id);
          pending.push(next);
        }
  }
  return target;
}

export function groupedBanished(entries: BanishedGame[]) {
  const games = entries.map((b) => ({
    ...b.game,
    metadataId:
      Number(b.keys.find((k) => k.startsWith("match:steam:"))?.slice(12)) ||
      undefined,
  }));
  const seen = new Set<string>(),
    result: BanishedGame[] = [];
  for (const entry of entries)
    if (!seen.has(entry.id)) {
      result.push(entry);
      for (const id of expandBlacklistTargets(games, [entry.id])) seen.add(id);
    }
  return result;
}
