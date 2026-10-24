import {
  recentSortAt,
  mergeSteamActivity,
  type ActivitySource,
} from "./play-activity";
import { consensusScore } from "./ratings";
import type { AccountSource, Game, GameCollection, Source } from "./types";
import { isCollectionFilter, type CollectionFilter } from "./collections";
import { tagKey } from "./tags";
import { searchIndex, searchText } from "./search-index";
import { mainExtraHours } from "./game-facts";
import { artworkIdentity } from "./artwork";
export const sortOptions = [
  ["title", "Title A–Z"],
  ["recent", "Recently played"],
  ["added", "Recently added"],
  ["year", "Release year"],
  ["hltb-asc", "Main + Extras · shortest first"],
  ["hltb-desc", "Main + Extras · longest first"],
  ["rating-desc", "Consensus rating · highest first"],
  ["rating-asc", "Consensus rating · lowest first"],
] as const;
export type Filter =
  "all" | "recent" | "favorites" | "hidden" | Source | CollectionFilter;
export function filterGames(
  games: Game[],
  filter: Filter,
  query: string,
  installed: boolean,
  tag: string,
  sort: string,
  collections: GameCollection[] = [],
  showHiddenGames = false,
  source = "",
  activitySource: ActivitySource = "all",
): Game[] {
  const members = isCollectionFilter(filter)
    ? new Set(collections.find((c) => c.id === filter.slice(11))?.gameIds ?? [])
    : null;
  const words = searchText(query).trim().split(/\s+/).filter(Boolean);
  const selectedTag = tagKey(tag);
  const result = games.filter((g) => {
    if (filter === "hidden" ? !g.hidden : g.hidden && !showHiddenGames)
      return false;
    if (source && !g.copies.some((c) => c.source === source)) return false;
    const matches =
      (filter === "all" ||
        filter === "hidden" ||
        (filter === "recent" && !!recentSortAt(g, activitySource)) ||
        (filter === "favorites" && g.favorite) ||
        (members?.has(g.id) ?? false) ||
        g.copies.some((c) => c.source === filter)) &&
      (!installed || g.copies.some((c) => c.installed)) &&
      (!selectedTag || searchIndex(g).tags.has(selectedTag));
    if (!matches || !words.length) return matches;
    const text = searchIndex(g).text;
    return words.every((w) => text.includes(w));
  });
  // Validate/provider-decode once per game, rather than inside O(n log n) comparisons.
  const times = sort.startsWith("hltb-")
    ? new Map(result.map((g) => [g.id, mainExtraHours(g)]))
    : sort.startsWith("rating-")
      ? new Map(result.map((g) => [g.id, consensusScore(g)]))
      : undefined;
  return result.sort((a, b) =>
    times
      ? (times.get(a.id) === undefined
          ? times.get(b.id) === undefined
            ? 0
            : 1
          : times.get(b.id) === undefined
            ? -1
            : (times.get(a.id)! - times.get(b.id)!) *
              (sort.endsWith("-desc") ? -1 : 1)) ||
        a.title.localeCompare(b.title)
      : sort === "recent"
        ? (recentSortAt(b, activitySource) ?? 0) -
            (recentSortAt(a, activitySource) ?? 0) ||
          a.title.localeCompare(b.title)
        : sort === "added"
          ? b.addedAt - a.addedAt
          : sort === "year"
            ? (b.year ?? 0) - (a.year ?? 0) || a.title.localeCompare(b.title)
            : a.title.localeCompare(b.title),
  );
}
export function mergeDiscovered(existing: Game[], incoming: Game[]): Game[] {
  if (!incoming.length) return existing;
  const result = existing.map((g) => ({
    ...g,
    copies: g.copies.map((c) => ({ ...c })),
  }));
  const byCopy = new Map(
    result.flatMap((g) => g.copies.map((c) => [c.id, g] as const)),
  );
  for (const game of incoming) {
    if (!game.copies.length) continue;
    // A scan may contain multiple copies, including copies already owned by
    // separate records. Update each owner without merging personal game data.
    const fallback = game.copies.map((c) => byCopy.get(c.id)).find(Boolean);
    if (!fallback) {
      const added = { ...game, copies: game.copies.map((c) => ({ ...c })) };
      result.push(added);
      for (const c of added.copies) byCopy.set(c.id, added);
      continue;
    }
    const touched = new Set<Game>();
    for (const copy of game.copies) {
      const found = byCopy.get(copy.id) ?? fallback;
      const old = found.copies.find((c) => c.id === copy.id);
      const updated = old
        ? {
            ...old,
            ...copy,
            executable: copy.executable ?? old.executable,
            launchUrl: copy.launchUrl ?? old.launchUrl,
            ownership: copy.ownership ?? old.ownership,
            installSizeBytes:
              copy.installSizeBytes ??
              (copy.installed && copy.installPath === old.installPath
                ? old.installSizeBytes
                : undefined),
            storePage:
              copy.storePage ??
              (copy.source === old.source && copy.sourceId === old.sourceId
                ? old.storePage
                : undefined),
          }
        : { ...copy };
      found.copies = old
        ? found.copies.map((c) => (c.id === copy.id ? updated : c))
        : [...found.copies, updated];
      byCopy.set(copy.id, found);
      touched.add(found);
    }
    for (const found of touched) {
      // Identity and user edits survive scans; only installation/art evidence changes.
      if (!found.metadataLocked && game.cover) found.cover = game.cover;
      if (!found.metadataLocked && game.hero) found.hero = game.hero;
    }
  }
  return retainUnchanged(existing, result);
}
export function preferredCopy(game: Game) {
  return (
    game.copies.find((c) => c.id === game.preferredCopy) ??
    game.copies.find((c) => c.installed && c.available !== false) ??
    game.copies[0]
  );
}

// An owned-library response says nothing about local installation. Never merge its
// installed:false over discovery, or replace a current game with an old snapshot.
export function mergeOwnedLibrary(
  existing: Game[],
  incoming: Game[],
  source: AccountSource,
): Game[] {
  if (
    incoming.some(
      (g) =>
        !g.copies.length ||
        g.copies.some((c) => c.source !== source || !c.ownership),
    )
  )
    throw new Error(
      "The source returned invalid ownership records. Your library was kept.",
    );
  const result = existing.map((g) => ({
    ...g,
    copies: g.copies.map((c) => ({ ...c })),
  }));
  const byCopy = new Map(
    result.flatMap((g) => g.copies.map((c) => [c.id, g] as const)),
  );
  for (const incomingGame of incoming) {
    const game = {
      ...incomingGame,
      storeMetadata: incomingGame.copies.map((c) => ({
        ...incomingGame.storeMetadata?.find(
          (m) => m.source === c.source && m.sourceId === c.sourceId,
        ),
        source: c.source,
        sourceId: c.sourceId,
        title: incomingGame.title,
        cover: incomingGame.cover,
        hero: incomingGame.hero,
        checkedAt: c.ownership?.verifiedAt ?? Date.now(),
      })),
    };
    const found = game.copies.map((c) => byCopy.get(c.id)).find(Boolean);
    if (!found) {
      const added = { ...game, copies: game.copies.map((c) => ({ ...c })) };
      result.push(added);
      for (const c of added.copies) byCopy.set(c.id, added);
      continue;
    }
    for (const copy of game.copies) {
      const old = found.copies.find((c) => c.id === copy.id);
      if (old) {
        old.ownership = copy.ownership;
        if (old.source === copy.source && old.sourceId === copy.sourceId) {
          old.storePage = copy.storePage ?? old.storePage;
          old.launchUrl = copy.launchUrl ?? old.launchUrl;
        }
      } else {
        found.copies.push({ ...copy });
        byCopy.set(copy.id, found);
      }
    }
    if (source === "steam")
      found.steamActivity = mergeSteamActivity(found, game.steamActivity ?? []);
    const sid = game.copies[0].sourceId;
    const placeholder = `${source === "itch" ? "itch.io" : source === "gog" ? "GOG" : source === "epic" ? "Epic" : source === "amazon" ? "Amazon" : "Steam"} game ${sid}`;
    const prior = found.storeMetadata?.find(
      (m) => m.source === source && m.sourceId === sid,
    );
    const oldIdentity = artworkIdentity(found);
    if (
      !found.metadataLocked &&
      !found.titleOverride &&
      game.title !== placeholder &&
      (found.title === placeholder ||
        found.title === prior?.title ||
        (source === "amazon" && !prior))
    ) {
      found.title = game.title;
      // A store correction retains an explicit artwork choice for the same copy.
      for (const slot of ["cover", "hero"] as const) {
        const choice = found.artwork?.[slot];
        if (choice?.identity === oldIdentity)
          found.artwork = {
            ...found.artwork,
            [slot]: { ...choice, identity: artworkIdentity(found) },
          };
      }
    }
    const refreshed = game.storeMetadata.map((m) => {
      const old = found.storeMetadata?.find(
        (p) => p.source === m.source && p.sourceId === m.sourceId,
      );
      return {
        ...m,
        cover:
          m.cover ||
          (m.source !== "amazon" || old?.coverKind === "portrait"
            ? old?.cover
            : undefined),
        hero: m.hero || old?.hero,
      };
    });
    if (!found.metadataLocked)
      found.storeMetadata = [
        ...(found.storeMetadata ?? []).filter(
          (m) =>
            !refreshed.some(
              (n) => m.source === n.source && m.sourceId === n.sourceId,
            ),
        ),
        ...refreshed,
      ];
    if (!found.metadataLocked && game.cover) found.cover = game.cover;
    if (!found.metadataLocked && game.hero) found.hero = game.hero;
    if (!found.metadataLocked && !found.description && game.description)
      found.description = game.description;
  }
  return retainUnchanged(existing, result);
}

// Keep presentation/artwork/enrichment caches warm for untouched rows after imports.
function retainUnchanged(existing: Game[], result: Game[]): Game[] {
  const before = new Map(existing.map((g) => [g.id, g]));
  const stable = result.map((next) => {
    const old = before.get(next.id);
    if (!old) return next;
    if (JSON.stringify(old.copies) === JSON.stringify(next.copies))
      next.copies = old.copies;
    if (
      JSON.stringify(old.storeMetadata) === JSON.stringify(next.storeMetadata)
    )
      next.storeMetadata = old.storeMetadata;
    return [...new Set([...Object.keys(old), ...Object.keys(next)])].every(
      (key) => old[key as keyof Game] === next[key as keyof Game],
    )
      ? old
      : next;
  });
  return stable.length === existing.length &&
    stable.every((g, i) => g === existing[i])
    ? existing
    : stable;
}
