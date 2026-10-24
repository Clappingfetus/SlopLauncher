import { copyIdentities } from "./enrichment-identity";
import { providerIds, type Game, type ProviderId } from "./types";

export const metadataProviders = providerIds.filter(
  (p) => p !== "achievements" && p !== "retroachievements",
);
// Credential revisions and provider expiry are deliberately absent. Connecting a
// service or reopening the app must not restart a completed discovery pass.
const identities = new WeakMap<Game, string>();
export function discoveryIdentity(game: Game): string {
  const cached = identities.get(game);
  if (cached) return cached;
  const key = JSON.stringify([
    game.id,
    game.title,
    game.metadataId,
    copyIdentities(game).sort((a, b) =>
      JSON.stringify(a).localeCompare(JSON.stringify(b)),
    ),
    Object.entries(game.providerIds ?? {})
      .filter(([, v]) => v != null)
      .sort(([a], [b]) => a.localeCompare(b)),
  ]);
  identities.set(game, key);
  return key;
}
export function baselineMetadata(game: Game): Game {
  return game.metadataScan
    ? game
    : {
        ...game,
        metadataScan: {
          identity: discoveryIdentity(game),
          attempted: [...metadataProviders],
        },
      };
}
export function needsDiscovery(game: Game, provider: ProviderId): boolean {
  return (
    game.metadataScan?.identity !== discoveryIdentity(game) ||
    !game.metadataScan.attempted.includes(provider)
  );
}
export function recordMetadataAttempt(game: Game, provider: ProviderId): Game {
  const identity = discoveryIdentity(game);
  const attempted =
    game.metadataScan?.identity === identity ? game.metadataScan.attempted : [];
  return attempted.includes(provider)
    ? game
    : {
        ...game,
        metadataScan: { identity, attempted: [...attempted, provider] },
      };
}
// Manual permissions belong to the current identity and are consumed once per
// provider. New arrivals still get their own automatic pass during a manual run.
export class ManualMetadataScan {
  private targets = new Map<
    string,
    { identity: string; attempted: Set<ProviderId> }
  >();
  start(games: Game[]) {
    this.targets = new Map(
      games.map((g) => [
        g.id,
        { identity: discoveryIdentity(g), attempted: new Set() },
      ]),
    );
  }
  contains(game: Game) {
    return this.targets.get(game.id)?.identity === discoveryIdentity(game);
  }
  allows(game: Game, provider: ProviderId) {
    const hit = this.targets.get(game.id);
    return (
      !!hit &&
      hit.identity === discoveryIdentity(game) &&
      !hit.attempted.has(provider)
    );
  }
  started(game: Game, provider: ProviderId) {
    this.targets.get(game.id)?.attempted.add(provider);
  }
  clear() {
    this.targets.clear();
  }
}
// Scheduling-only patches omit cached metadata, artwork and personal text. The
// native enrichment boundary validates identity and merges only allowed fields.
export function metadataAttemptPatch(game: Game): Game {
  return {
    id: game.id,
    title: game.title,
    copies: game.copies,
    metadataId: game.metadataId,
    providerIds: game.providerIds,
    metadataScan: game.metadataScan,
    genres: [],
    screenshots: [],
    favorite: false,
    status: "unplayed",
    notes: "",
    addedAt: 0,
  };
}
