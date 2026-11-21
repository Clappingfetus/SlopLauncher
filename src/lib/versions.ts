import { summarizeActivity, type ActivitySummary } from "./play-activity";
import { ratingRows } from "./ratings";
import { sourceNames, type Game, type Copy, type Settings } from "./types";
import { currentResult, steamId } from "./enrichment-identity";
import { preferredArtwork } from "./store-artwork";
import { isEmulated, selectedArtwork } from "./artwork";
const sameStringList = (a?: string[], b?: string[]) =>
  a === b ||
  (!!a && !!b && a.length === b.length && a.every((v, i) => v === b[i]));
const sameFallbacks = (
  a: Game["artworkFallbacks"],
  b: Game["artworkFallbacks"],
) =>
  a === b ||
  (sameStringList(a?.cover, b?.cover) && sameStringList(a?.hero, b?.hero));
const sameActivity = (
  a: ActivitySummary | undefined,
  b: ActivitySummary | undefined,
) =>
  a === b ||
  (!!a &&
    !!b &&
    a.launcherLastAt === b.launcherLastAt &&
    a.launcherLaunchCount === b.launcherLaunchCount &&
    a.steam?.lastPlayedAt === b.steam?.lastPlayedAt &&
    a.steam?.totalMinutes === b.steam?.totalMinutes &&
    a.steam?.recentMinutes === b.steam?.recentMinutes &&
    a.steam?.checkedAt === b.steam?.checkedAt &&
    a.steam?.recentCheckedAt === b.steam?.recentCheckedAt);
const reviewPresentationCache = new WeakMap<Game, Game>();
export function gameContextIdentity(game: Game, copy: Copy | undefined) {
  return JSON.stringify([
    game.id,
    game.title,
    game.metadataId,
    game.providerIds,
    copy?.id,
    copy?.source,
    copy?.sourceId,
    copy?.installPath,
    copy?.executable,
    copy?.installed,
    copy?.available,
  ]);
}
export interface GameVersion {
  game: Game;
  copy: Copy;
}
interface VersionOption extends GameVersion {
  key: string;
  label: string;
  detail?: string;
}
export function versionKey(gameId: string, copyId: string) {
  return JSON.stringify([gameId, copyId]);
}
export function versionOptions(versions: GameVersion[]): VersionOption[] {
  const counts = new Map<string, number>();
  const options = versions.map(({ game, copy }) => {
    const source = sourceNames[copy.source] ?? copy.source;
    const status =
      copy.available === false
        ? " · Unavailable"
        : copy.installed
          ? " · Installed"
          : "";
    const label = source + status;
    counts.set(label, (counts.get(label) ?? 0) + 1);
    return { key: versionKey(game.id, copy.id), label, game, copy };
  });
  const compactCounts = new Map<string, number>();
  const labeled = options.map<VersionOption>((option) => {
    if (counts.get(option.label) === 1) return option;
    const { game, copy } = option;
    const detail =
      copy.emulation?.gamePath ||
      copy.executable ||
      copy.installPath ||
      copy.sourceId ||
      game.title;
    const basename = detail.split(/[\\/]/).filter(Boolean).at(-1) ?? detail;
    const label = `${basename} · ${option.label}`;
    compactCounts.set(label, (compactCounts.get(label) ?? 0) + 1);
    return { ...option, label, detail };
  });
  return labeled.map((option) =>
    option.detail && (compactCounts.get(option.label) ?? 0) > 1
      ? { ...option, label: `${option.label} · ${option.detail}` }
      : option,
  );
}
const titleKey = (s: string) =>
  s
    .replace(/[™®©]/g, "")
    .normalize("NFKC")
    .toLowerCase()
    .replace(/[™®©]/g, "")
    .replace(/&/g, " and ")
    .replace(/['‘’]/g, "")
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
const domain = (g: Game) =>
  !g.copies.length
    ? `unassigned:${g.id}`
    : g.copies.every((c) => !!c.emulation)
      ? `emu:${[...new Set(g.copies.map((c) => c.emulation?.platform))].sort().join(",")}`
      : g.copies.some((c) => !!c.emulation)
        ? `mixed:${g.id}`
        : "pc";
interface MatchIdentity {
  platform: string;
  igdb: number | undefined;
  keys: string[];
  catalogs: Map<string, Set<string>>;
  title: string;
}
function matchIdentity(g: Game, revision: number): MatchIdentity {
  const platform = domain(g),
    id = g.providerIds?.igdb ?? currentResult(g, "igdb", revision)?.data?.id,
    igdb =
      typeof id === "number" && Number.isSafeInteger(id) && id > 0
        ? id
        : undefined,
    catalogs = new Map<string, Set<string>>();
  const add = (source: string, value: string) => {
    const ids = catalogs.get(source) ?? new Set<string>();
    ids.add(value);
    catalogs.set(source, ids);
  };
  if (igdb) add("igdb", String(igdb));
  for (const copy of g.copies) {
    if (copy.source !== "manual" && copy.source !== "emulated" && copy.sourceId)
      add(copy.source, copy.sourceId);
  }
  // Native ownership is authoritative when a metadata match disagrees.
  if (!catalogs.has("steam") && steamId(g)) add("steam", String(steamId(g)));
  return {
    platform,
    igdb,
    title: titleKey(g.title),
    catalogs,
    // Wiki articles can cover multiple editions; they are not ownership IDs.
    keys: [...catalogs].flatMap(([source, ids]) =>
      [...ids].map((id) => `${source}:${id}`),
    ),
  };
}
export function createVersionResolver() {
  const cache = new WeakMap<
    Game,
    { revision: number; identity: MatchIdentity }
  >();
  return (
    games: Game[],
    settings: Settings,
    indexDuplicates = true,
    revision = 0,
  ) =>
    versionLibrary(games, settings, indexDuplicates, revision, (g, r) => {
      const hit = cache.get(g);
      if (hit?.revision === r) return hit.identity;
      const identity = matchIdentity(g, r);
      cache.set(g, { revision: r, identity });
      return identity;
    });
}
// Presentation-only grouping: retain every original record and its launch copy.
// Catalog conflicts must win over title similarity; never merge ownership here.
function countLibraryGames(
  games: Game[],
  showHidden: boolean,
  groupId?: (id: string) => string,
) {
  const visible = 1,
    hidden = 2,
    favorite = 4,
    installed = 8;
  const counted = groupId ? new Map<string, number>() : undefined;
  const counts = { library: 0, hidden: 0, favorites: 0, installed: 0 };
  for (const game of games) {
    const isVisible = showHidden || !game.hidden;
    let flags = game.hidden ? hidden : 0;
    if (isVisible) {
      flags |= visible;
      if (game.favorite) flags |= favorite;
      if (game.copies.some((copy) => copy.installed)) flags |= installed;
    }
    if (counted && groupId) {
      // Each edition can contribute different flags to the same group.
      const key = groupId(game.id);
      const previous = counted.get(key) ?? 0;
      counted.set(key, previous | flags);
      flags &= ~previous;
    }
    if (flags & visible) counts.library++;
    if (flags & hidden) counts.hidden++;
    if (flags & favorite) counts.favorites++;
    if (flags & installed) counts.installed++;
  }
  return counts;
}
export function versionLibrary(
  games: Game[],
  settings: Settings,
  indexDuplicates = true,
  revision = 0,
  readIdentity = matchIdentity,
) {
  const byId = new Map<string, Game>();
  for (const game of games) byId.set(game.id, game);
  if (!indexDuplicates)
    return {
      members: (id: string) => (byId.has(id) ? [byId.get(id)!] : []),
      versions: (id: string) =>
        (byId.get(id)?.copies ?? []).map((copy) => ({
          game: byId.get(id)!,
          copy,
        })),
      collapse: (list: Game[]) => list,
      count: (list: Game[]) => list.length,
      counts: (list: Game[], showHidden: boolean) =>
        countLibraryGames(list, showHidden),
      choose: (list: Game[]) => list[0],
      withArtwork: (game: Game) => game,
      withPresentation: (game: Game) => ({
        ...game,
        reviewSummary: ratingRows([game]),
      }),
    };
  // Only non-root records need parent entries.
  const parent = new Map<string, string>();
  const facts = new Map<string, Map<string, Set<string>>>();
  const root = (id: string): string => {
    let r = id;
    let next = parent.get(r);
    while (next !== undefined) {
      r = next;
      next = parent.get(r);
    }
    while (id !== r) {
      const next = parent.get(id)!;
      parent.set(id, r);
      id = next;
    }
    return r;
  };
  const join = (a: string, b: string) => {
    a = root(a);
    b = root(b);
    if (a === b) return;
    const left = facts.get(a)!,
      right = facts.get(b)!;
    // Check the entire component, so a title-only copy cannot bridge two
    // contradictory catalog matches through transitive unions.
    for (const [source, ids] of left) {
      const other = right.get(source);
      if (
        other &&
        (ids.size !== other.size || [...ids].some((id) => !other.has(id)))
      )
        return;
    }
    parent.set(b, a);
    // Identities can be reused by the next index. Only merged roots own new facts.
    facts.set(a, new Map([...left, ...right]));
  };
  const compatible = (list: Game[]) => {
    const combined = new Map<string, Set<string>>();
    for (const g of list) {
      for (const [source, ids] of facts.get(root(g.id))!) {
        const prior = combined.get(source);
        if (
          prior &&
          (ids.size !== prior.size || [...ids].some((id) => !prior.has(id)))
        )
          return false;
        combined.set(source, ids);
      }
    }
    return true;
  };
  const strong = new Map<string, Game | Game[]>(),
    titles = new Map<string, Game | Game[]>();
  const addToBucket = (
    buckets: Map<string, Game | Game[]>,
    key: string,
    game: Game,
  ) => {
    const bucket = buckets.get(key);
    if (Array.isArray(bucket)) bucket.push(game);
    else buckets.set(key, bucket ? [bucket, game] : game);
  };
  for (const g of games) {
    const identity = readIdentity(g, revision);
    facts.set(g.id, identity.catalogs);
    for (const key of identity.keys)
      addToBucket(strong, identity.platform + ":" + key, g);
    if (identity.title)
      addToBucket(titles, identity.platform + ":" + identity.title, g);
  }
  for (const list of strong.values()) {
    // Reject an ambiguous bucket as a whole, rather than assigning an
    // unknown copy to whichever conflicting candidate happened to arrive first.
    if (!Array.isArray(list) || !compatible(list)) continue;
    for (const g of list.slice(1)) join(list[0].id, g.id);
  }
  for (const list of titles.values()) {
    if (!Array.isArray(list)) continue;
    // Repeated same-store titles are ambiguous unless a catalog identity already joined them.
    const owners = new Map<string, Set<string>>();
    for (const g of list)
      for (const c of g.copies) {
        const set = owners.get(c.source) ?? new Set<string>();
        set.add(root(g.id));
        owners.set(c.source, set);
      }
    if ([...owners.values()].some((set) => set.size > 1)) continue;
    const years = new Set(list.map((g) => g.year).filter(Boolean));
    const ids = new Set(
      list.map((g) => readIdentity(g, revision).igdb).filter(Boolean),
    );
    if (years.size > 1 || ids.size > 1 || !compatible(list)) continue;
    for (const g of list.slice(1)) join(list[0].id, g.id);
  }
  const groups = new Map<string, Game[]>();
  for (const g of games) {
    const key = root(g.id),
      group = groups.get(key) ?? [];
    group.push(g);
    groups.set(key, group);
  }
  const members = (id: string) => (byId.has(id) ? groups.get(root(id))! : []);
  const groupArtwork = new Map<string, { cover: string[]; hero: string[] }>();
  const decorated = new WeakMap<Game, Game>();
  function withArtwork(game: Game): Game {
    if (game.metadataLocked || isEmulated(game) || !byId.has(game.id))
      return game;
    const hit = decorated.get(game);
    if (hit) return hit;
    const key = root(game.id);
    let assets = groupArtwork.get(key);
    if (!assets) {
      const all = members(game.id);
      assets = {
        cover: preferredArtwork(all, "cover", revision),
        hero: preferredArtwork(all, "hero", revision),
      };
      groupArtwork.set(key, assets);
    }
    const patch: Partial<Game> = {};
    const fallbacks: Game["artworkFallbacks"] = {};
    for (const slot of ["cover", "hero"] as const) {
      if (selectedArtwork(game, slot)) continue;
      if (assets[slot].length) {
        if (assets[slot][0] !== game[slot]) patch[slot] = assets[slot][0];
        if (assets[slot].length > 1) fallbacks[slot] = assets[slot].slice(1);
      }
    }
    if (Object.keys(fallbacks).length) patch.artworkFallbacks = fallbacks;
    const shown = Object.keys(patch).length ? { ...game, ...patch } : game;
    decorated.set(game, shown);
    return shown;
  }
  // Card counts and the details picker share one list per duplicate group.
  // Scope this cache to the current index so imports/rematches cannot go stale.
  const versionGroups = new Map<string, GameVersion[]>();
  const versions = (id: string): GameVersion[] => {
    if (!byId.has(id)) return [];
    const key = root(id);
    let list = versionGroups.get(key);
    if (!list) {
      list = members(id).flatMap((game) =>
        game.copies.map((copy) => ({ game, copy })),
      );
      versionGroups.set(key, list);
    }
    return list;
  };
  function choose(candidates: Game[]): Game {
    if (candidates.length === 1 && candidates[0].copies.length <= 1)
      return candidates[0];
    const allowed = new Set(candidates.map((g) => g.id)),
      all = members(candidates[0].id),
      options = candidates.flatMap((game) =>
        game.copies.map((copy) => ({ game, copy })),
      );
    const prefs = all
      .map((g) => settings.versionPreferences?.[g.id])
      .filter((p) => !!p && allowed.has(p.gameId))
      .sort((a, b) => b!.selectedAt - a!.selectedAt);
    let chosen: GameVersion | undefined;
    for (const pref of prefs) {
      chosen = options.find(
        (v) => v.game.id === pref!.gameId && v.copy.id === pref!.copyId,
      );
      if (chosen) break;
    }
    chosen ??= options.find((v) => v.game.preferredCopy === v.copy.id);
    chosen ??= settings.hideDuplicateGames
      ? (options.find(
          (v) =>
            v.copy.source === (settings.preferredSource ?? "steam") &&
            v.copy.installed &&
            v.copy.available !== false,
        ) ??
        options.find(
          (v) => v.copy.source === (settings.preferredSource ?? "steam"),
        ))
      : undefined;
    chosen ??=
      options.find((v) => v.copy.installed && v.copy.available !== false) ??
      options[0];
    if (!chosen) return candidates[0];
    const natural =
      chosen.game.copies.find((c) => c.id === chosen!.game.preferredCopy) ??
      chosen.game.copies.find((c) => c.installed && c.available !== false) ??
      chosen.game.copies[0];
    return natural?.id === chosen.copy.id
      ? chosen.game
      : { ...chosen.game, preferredCopy: chosen.copy.id };
  }
  function collapse(list: Game[]): Game[] {
    if (!settings.hideDuplicateGames) return list;
    const buckets = new Map<string, Game | Game[]>();
    for (const game of list) {
      const key = root(game.id);
      const bucket = buckets.get(key);
      if (Array.isArray(bucket)) bucket.push(game);
      else buckets.set(key, bucket ? [bucket, game] : game);
    }
    const collapsed: Game[] = [];
    for (const bucket of buckets.values()) {
      if (Array.isArray(bucket)) collapsed.push(choose(bucket));
      else collapsed.push(bucket.copies.length <= 1 ? bucket : choose([bucket]));
    }
    return collapsed;
  }
  const reviewGroups = new Map<string, ReturnType<typeof ratingRows>>();
  const activityGroups = new Map<string, ActivitySummary | undefined>();
  const presentations = new WeakMap<Game, Game>();
  function withPresentation(game: Game): Game {
    const hit = presentations.get(game);
    if (hit) return hit;
    const key = root(game.id);
    let rows = reviewGroups.get(key);
    if (!rows) {
      rows = ratingRows(members(game.id));
      reviewGroups.set(key, rows);
    }
    if (!activityGroups.has(key))
      activityGroups.set(key, summarizeActivity(members(game.id)));
    const activitySummary = activityGroups.get(key);
    const artwork = withArtwork(game);
    const previous = reviewPresentationCache.get(game);
    if (
      previous &&
      previous.cover === artwork.cover &&
      previous.hero === artwork.hero &&
      sameActivity(previous.activitySummary, activitySummary) &&
      sameFallbacks(previous.artworkFallbacks, artwork.artworkFallbacks) &&
      previous.reviewSummary?.every(
        (row, i) =>
          row.rating === rows![i].rating &&
          row.checkedAt === rows![i].checkedAt &&
          row.status === rows![i].status &&
          row.message === rows![i].message &&
          row.stale === rows![i].stale,
      )
    ) {
      presentations.set(game, previous);
      return previous;
    }
    const shown = { ...artwork, reviewSummary: rows, activitySummary };
    reviewPresentationCache.set(game, shown);
    presentations.set(game, shown);
    return shown;
  }
  function count(list: Game[]): number {
    if (!settings.hideDuplicateGames) return list.length;
    const groups = new Set<string>();
    for (const game of list) groups.add(root(game.id));
    return groups.size;
  }
  const counts = (list: Game[], showHidden: boolean) =>
    countLibraryGames(
      list,
      showHidden,
      settings.hideDuplicateGames ? root : undefined,
    );
  return { members, versions, collapse, count, counts, choose, withArtwork, withPresentation };
}
