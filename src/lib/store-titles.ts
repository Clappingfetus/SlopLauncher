import type { Game } from "./types";
import { currentResult } from "./enrichment-identity";
import { artworkIdentity } from "./artwork";

// Only an explicitly separated promotional date RANGE, never a release year,
// sequel number, single date or subtitle. This applies to any Amazon product.
const month =
  "(?:January|February|March|April|May|June|July|August|September|October|November|December)";
const day = "(?:[1-9]|[12][0-9]|3[01])";
const promotion = new RegExp(
  `:\\s*${month}\\s+${day}\\s*[-–—]\\s*(?:${month}\\s+)?${day}(?:,?\\s+(?:19|20)\\d{2})?\\s*$`,
  "i",
);
export function cleanAmazonTitle(title: string) {
  return title.replace(promotion, "").trim() || title;
}
export function reconcileStoreTitles(games: Game[], revision = 0): Game[] {
  let changed = false;
  const result = games.map((game) => {
    if (
      game.metadataLocked ||
      game.titleOverride ||
      !game.copies.some((c) => c.source === "amazon")
    )
      return game;
    // IGDB only returns ready after a unique title/platform or catalog-ID match.
    const igdb = currentResult(game, "igdb", revision);
    const canonical =
      igdb?.status === "ready" ? igdb.data?.title?.trim() : undefined;
    const cleaned = cleanAmazonTitle(game.title);
    const comparable = (value: string) =>
      value.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");
    const title =
      canonical && comparable(canonical) !== comparable(cleaned)
        ? canonical
        : cleaned;
    if (title === game.title) return game;
    changed = true;
    const next = { ...game, title };
    const oldIdentity = artworkIdentity(game);
    for (const slot of ["cover", "hero"] as const) {
      const choice = game.artwork?.[slot];
      if (choice?.identity === oldIdentity)
        next.artwork = {
          ...next.artwork,
          [slot]: { ...choice, identity: artworkIdentity(next) },
        };
    }
    return next;
  });
  return changed ? result : games;
}
