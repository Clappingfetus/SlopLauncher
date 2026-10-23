import type { Game } from "./types";
import { currentResult } from "./enrichment-identity";
import { isEmulated, selectedArtwork } from "./artwork";

const priorities: Record<string, number> = {
  steam: 0,
  gog: 1,
  epic: 2,
  amazon: 3,
  itch: 4,
  steamgriddb: 5,
  igdb: 6,
};
export function artworkSource(url: string): string | undefined {
  try {
    const host = new URL(url).hostname;
    if (
      /^(?:cdn|shared)\.(?:akamai|cloudflare|fastly)\.steamstatic\.com$/.test(
        host,
      )
    )
      return "steam";
    if (/^images(?:-[1-4])?\.gog-statics\.com$/.test(host)) return "gog";
    if (
      [
        "cdn1.epicgames.com",
        "cdn2.unrealengine.com",
        "cdn1.unrealengine.com",
        "library-assets.epicgames.com",
      ].includes(host)
    )
      return "epic";
    if (
      ["m.media-amazon.com", "images-na.ssl-images-amazon.com"].includes(host)
    )
      return "amazon";
    if (["img.itch.zone", "img.itch.io"].includes(host)) return "itch";
    if (
      /^cdn[12]?\.steamgriddb\.com$/.test(host) ||
      (host === "s3.amazonaws.com" &&
        new URL(url).pathname.startsWith("/steamgriddb/"))
    )
      return "steamgriddb";
    if (host === "images.igdb.com") return "igdb";
  } catch {
    /* Existing local/manual images retain their fallback position. */
  }
}
export function artworkCandidates(
  game: Game,
  slot: "cover" | "hero",
  revision = 0,
) {
  if (isEmulated(game) || game.metadataLocked) return [];
  const candidates: { url: string; rank: number; checkedAt: number }[] = [];
  const add = (url: string | undefined, source?: string, checkedAt = 0) => {
    if (
      slot === "cover" &&
      (source ?? (url && artworkSource(url))) === "amazon" &&
      !game.storeMetadata?.some(
        (m) =>
          m.source === "amazon" &&
          m.coverKind === "portrait" &&
          m.cover === url &&
          game.copies.some(
            (c) => c.source === m.source && c.sourceId === m.sourceId,
          ),
      )
    )
      return;
    if (url?.trim())
      candidates.push({
        url,
        rank: priorities[source ?? artworkSource(url) ?? ""] ?? 4,
        checkedAt,
      });
  };
  for (const metadata of game.storeMetadata ?? [])
    if (
      game.copies.some(
        (copy) =>
          copy.source === metadata.source &&
          copy.sourceId === metadata.sourceId,
      )
    )
      add(metadata[slot], metadata.source, metadata.checkedAt);
  if (game.copies.some((copy) => copy.source === "steam"))
    add(
      currentResult(game, "steam", revision)?.data?.[slot],
      "steam",
      currentResult(game, "steam", revision)?.checkedAt,
    );
  if (game.copies.some((copy) => copy.source === "gog"))
    add(
      currentResult(game, "stores", revision)?.data?.[slot],
      "gog",
      currentResult(game, "stores", revision)?.checkedAt,
    );
  const provenance = game.storeMetadata?.filter((m) => m[slot] === game[slot]);
  const stale =
    provenance?.length &&
    !provenance.some((m) =>
      game.copies.some(
        (c) => c.source === m.source && c.sourceId === m.sourceId,
      ),
    );
  if (!selectedArtwork(game, slot) && !stale) add(game[slot]);
  add(
    currentResult(game, "steamgriddb", revision)?.data?.[slot],
    "steamgriddb",
  );
  add(currentResult(game, "igdb", revision)?.data?.[slot], "igdb");
  return candidates.sort(
    (a, b) => a.rank - b.rank || b.checkedAt - a.checkedAt,
  );
}
export function preferredArtwork(
  games: Game[],
  slot: "cover" | "hero",
  revision = 0,
) {
  return [
    ...new Set(
      games
        .flatMap((game) => artworkCandidates(game, slot, revision))
        .sort(
          (a, b) =>
            a.rank - b.rank ||
            b.checkedAt - a.checkedAt ||
            a.url.localeCompare(b.url),
        )
        .map((candidate) => candidate.url),
    ),
  ];
}

/** Refresh old importer records, including copies discovered after startup. */
export function needsAmazonPortraitRefresh(games: Game[]): boolean {
  return games.some(
    (game) =>
      !game.metadataLocked &&
      game.copies.some(
        (copy) =>
          copy.source === "amazon" &&
          !game.storeMetadata?.some(
            (m) =>
              m.source === "amazon" &&
              m.sourceId === copy.sourceId &&
              m.coverKind === "portrait",
          ),
      ),
  );
}
