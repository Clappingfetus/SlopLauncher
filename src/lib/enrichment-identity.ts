import type { Game, ProviderId, EnrichmentRecord } from "./types";
import { preferredCopy } from "./library";
export function copyIdentities(game: Game) {
  return game.copies.map((c) =>
    c.emulation
      ? [c.source, c.sourceId, c.emulation.platform, c.emulation.gamePath]
      : [c.source, c.sourceId],
  );
}

export function steamId(game: Game): number | undefined {
  const id =
    game.metadataId ??
    Number(game.copies.find((c) => c.source === "steam")?.sourceId);
  return Number.isSafeInteger(id) && id > 0 ? id : undefined;
}
export function identity(
  game: Game,
  provider: ProviderId,
  revision = 0,
): string {
  if (provider === "retroachievements") {
    const copy = preferredCopy(game);
    return JSON.stringify([
      1,
      game.id,
      game.title,
      copy?.id,
      copy?.source,
      copy?.sourceId,
      copy?.emulation?.platform,
      copy?.emulation?.gamePath,
      copy?.emulation?.retroAchievementsGameId,
      revision,
    ]);
  }
  if (provider === "achievements") {
    const copy = preferredCopy(game);
    return JSON.stringify([
      1,
      game.id,
      game.title,
      game.metadataId,
      copy?.id,
      copy?.source,
      copy?.sourceId,
      revision,
    ]);
  }
  const copies = copyIdentities(game).sort((a, b) =>
    JSON.stringify(a).localeCompare(JSON.stringify(b)),
  );
  const pcgw =
    provider === "hltb" ? currentResult(game, "pcgw")?.data?.hltbId : undefined;
  return JSON.stringify([
    provider === "steam"
      ? 3
      : provider === "stores"
        ? 3
        : provider === "steamgriddb"
          ? 4
          : provider === "pcgw"
            ? 4
            : provider === "igdb"
              ? 3
              : 2,
    game.id,
    game.title,
    steamId(game),
    copies,
    Object.fromEntries(
      Object.entries(game.providerIds ?? {})
        .filter(([, value]) => value !== undefined && value !== null)
        .sort(([a], [b]) => a.localeCompare(b)),
    ),
    provider,
    provider === "igdb" ||
    provider === "steamgriddb" ||
    provider === "screenscraper"
      ? revision
      : 0,
    provider.startsWith("reviews-")
      ? [
          currentResult(game, "pcgw")?.data?.reviewLinks,
          game.copies.map((c) => c.storePage),
        ]
      : provider === "nexus"
        ? currentResult(game, "pcgw")?.data?.nexusDomains
        : pcgw,
  ]);
}
export function currentResult(
  game: Game,
  provider: ProviderId,
  revision = 0,
): EnrichmentRecord | undefined {
  const result = game.enrichment?.[provider];
  if (!result) return undefined;
  return result?.fingerprint === identity(game, provider, revision)
    ? result
    : undefined;
}
