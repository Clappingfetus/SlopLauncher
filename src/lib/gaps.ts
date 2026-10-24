import { exactReleaseDate } from "./release-date";
import { providerIds } from "./types";
import type { Game, ProviderId, ProviderStatus } from "./types";
import { presentGame, currentResult, steamId } from "./enrichment";
import { isEmulated } from "./artwork";

export function missingFields(game: Game) {
  if (game.metadataLocked) return [];
  return [
    !game.cover?.trim() && "box art",
    !game.hero?.trim() && "background",
    !game.description?.trim() && "description",
    !game.developer?.trim() && "developer",
    !game.publisher?.trim() && "publisher",
    exactReleaseDate(game.releasedAt) === undefined && "release date",
    !game.genres.length && "genres",
    !game.screenshots.length && "screenshots",
  ].filter(Boolean) as string[];
}
export function nativeProvider(game: Game): ProviderId | undefined {
  if (isEmulated(game)) return "screenscraper";
  if (game.copies.some((c) => c.source === "steam")) return "steam";
  if (
    game.copies.some((c) => c.source === "gog" && /^[1-9]\d*$/.test(c.sourceId))
  )
    return "stores";
  if (steamId(game) && game.copies.every((c) => c.source === "manual"))
    return "steam";
  return undefined;
}
export function gapProviderEnabled(
  game: Game,
  provider: ProviderId,
  status: ProviderStatus,
): boolean {
  if (
    provider.startsWith("reviews-") ||
    provider === "achievements" ||
    provider === "retroachievements"
  )
    return false;
  if (game.metadataLocked) return false;
  const shown = presentGame(game, status.revision);
  if (!missingFields(shown).length) return false;
  const native = nativeProvider(game);
  const triedNative =
    !native ||
    (native === "screenscraper" && !status.screenScraperConfigured) ||
    !!currentResult(game, native, status.revision);
  switch (provider) {
    case "steam":
      return native === "steam";
    case "stores":
      return native === "stores";
    case "screenscraper":
      return isEmulated(game) && !!status.screenScraperConfigured;
    case "steamgriddb":
      return (
        !isEmulated(game) &&
        !shown.cover?.trim() &&
        status.steamGridDbConfigured &&
        triedNative
      );
    case "igdb":
      return status.igdbConfigured && triedNative;
    default:
      return true;
  }
}
// One policy computation per immutable game revision, shared across providers.
export function createGapPolicy(
  present = (g: Game, r: number) => presentGame(g, r),
) {
  const cache = new WeakMap<Game, { key: string; enabled: Set<ProviderId> }>();
  return (game: Game, provider: ProviderId, status: ProviderStatus) => {
    const key = [
      status.revision,
      status.igdbConfigured,
      status.steamGridDbConfigured,
      status.screenScraperConfigured,
    ].join(":");
    let hit = cache.get(game);
    if (!hit || hit.key !== key) {
      const enabled = new Set<ProviderId>(),
        shown = present(game, status.revision);
      if (!game.metadataLocked && missingFields(shown).length) {
        const native = nativeProvider(game);
        const tried =
          !native ||
          (native === "screenscraper" && !status.screenScraperConfigured) ||
          !!currentResult(game, native, status.revision);
        for (const p of providerIds) {
          if (
            p.startsWith("reviews-") ||
            p === "achievements" ||
            p === "retroachievements"
          )
            continue;
          const allow =
            p === "steam"
              ? native === "steam"
              : p === "stores"
                ? native === "stores"
                : p === "screenscraper"
                  ? isEmulated(game) && !!status.screenScraperConfigured
                  : p === "igdb"
                    ? status.igdbConfigured && tried
                    : p === "steamgriddb"
                      ? !isEmulated(game) &&
                        !shown.cover?.trim() &&
                        status.steamGridDbConfigured &&
                        tried
                      : true;
          if (allow) enabled.add(p);
        }
      }
      hit = { key, enabled };
      cache.set(game, hit);
    }
    return hit.enabled.has(provider);
  };
}
export const gapProviderOrder: Record<ProviderId, number> = {
  retroachievements: -3,
  achievements: -3,
  steam: 0,
  stores: 0,
  screenscraper: 1,
  igdb: 2,
  steamgriddb: 3,
  pcgw: 4,
  hltb: 5,
  nexus: 6,
  "reviews-steam": 7,
  "reviews-gog": 8,
  "reviews-metacritic": 9,
  "reviews-opencritic": 9,
  "reviews-itch": 8,
};

// These resources enrich the shared library, never a filtered view of it.
export function isLibraryResource(provider: ProviderId) {
  return (
    ["pcgw", "hltb", "nexus"].includes(provider) ||
    provider.startsWith("reviews-")
  );
}
export function libraryResourceEnabled(game: Game, provider: ProviderId) {
  if (provider === "reviews-steam") return !!steamId(game);
  if (provider === "reviews-itch")
    return game.copies.some((c) => c.source === "itch" && !!c.storePage);
  if (provider === "reviews-metacritic")
    return !!game.providerIds?.metacriticSlug || !!currentResult(game, "pcgw");
  if (provider === "reviews-opencritic")
    return !!game.providerIds?.opencriticPath || !!currentResult(game, "pcgw");
  return isLibraryResource(provider);
}
