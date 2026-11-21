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
function steamMatchId(game: Game) {
  const owned = Number(
    game.copies.find((c) => c.source === "steam" && /^\d+$/.test(c.sourceId))
      ?.sourceId,
  );
  return Number.isSafeInteger(owned) && owned > 0 ? owned : game.metadataId;
}
function titleKey(title: string) {
  return `title:${title.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "")}`;
}
export function blacklistKeys(game: Game): string[] {
  const steam = steamMatchId(game);
  return [
    `game:${game.id}`,
    titleKey(game.title),
    ...game.copies.map((c) => `copy:${c.source}:${c.sourceId}`),
    ...(steam ? [`copy:steam:${steam}`, `match:steam:${steam}`] : []),
  ];
}
export function allowedGames(games: Game[], entries: BanishedGame[]) {
  if (!entries.length) return games;
  if (entries.length === 1)
    return games.filter((game) => !blacklistMatches(game, entries[0].keys));

  const identities = new Set<string>();
  const titles = new Map<string, Set<string | undefined>>();
  for (const { keys } of entries) {
    const match = keys.find((key) => key.startsWith("match:steam:"));
    for (const key of keys) {
      if (key.startsWith("title:")) {
        const matches = titles.get(key) ?? new Set<string | undefined>();
        matches.add(match);
        titles.set(key, matches);
      } else {
        identities.add(key);
      }
    }
  }
  return games.filter((game) => {
    if (identities.has(`game:${game.id}`)) return false;
    for (const copy of game.copies)
      if (identities.has(`copy:${copy.source}:${copy.sourceId}`)) return false;
    const steam = steamMatchId(game);
    const match = steam ? `match:steam:${steam}` : undefined;
    if (match && (identities.has(match) || identities.has(`copy:steam:${steam}`)))
      return false;
    const matches = titles.size ? titles.get(titleKey(game.title)) : undefined;
    if (!matches) return true;
    return !!match && !matches.has(undefined) && !matches.has(match);
  });
}
export function blacklistMatches(game: Game, blocked: string[]) {
  return matchesBlacklistKeys(blacklistKeys(game), blocked);
}
function matchesBlacklistKeys(keys: string[], blocked: string[]) {
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
function blacklistTraversal(games: Game[]) {
  type Entry = { id: string; keys: string[] };
  const byId = new Map<string, Entry>();
  const byKey = new Map<string, Entry[]>();
  for (const game of games) {
    const entry = { id: game.id, keys: blacklistKeys(game) };
    byId.set(entry.id, entry);
    for (const key of entry.keys) {
      const matches = byKey.get(key);
      if (matches) matches.push(entry);
      else byKey.set(key, [entry]);
    }
  }
  return (ids: string[], target: Set<string>) => {
    const pending = ids.map((id) => byId.get(id)).filter((entry) => !!entry);
    while (pending.length) {
      const { keys } = pending.pop()!;
      for (const key of keys)
        for (const next of byKey.get(key) ?? [])
          if (!target.has(next.id) && matchesBlacklistKeys(next.keys, keys)) {
            target.add(next.id);
            pending.push(next);
          }
    }
    return target;
  };
}

export function expandBlacklistTargets(games: Game[], ids: string[]) {
  return blacklistTraversal(games)(ids, new Set(ids));
}

export function groupedBanished(entries: BanishedGame[]) {
  if (entries.length < 2) return entries;
  const games = entries.map((b) => ({
    ...b.game,
    metadataId:
      Number(b.keys.find((k) => k.startsWith("match:steam:"))?.slice(12)) ||
      undefined,
  }));
  const expand = blacklistTraversal(games);
  const seen = new Set<string>(),
    result: BanishedGame[] = [];
  for (const entry of entries)
    if (!seen.has(entry.id)) {
      result.push(entry);
      seen.add(entry.id);
      expand([entry.id], seen);
    }
  return result;
}
