import type { Game, ArtworkChoice } from "./types";

// Personal fields, launch preferences and credential revisions do not rematch a game.
export function artworkIdentity(game: Game): string {
  return JSON.stringify([
    game.id,
    game.title,
    game.metadataId ?? null,
    game.copies
      .map((c) => [
        c.source,
        c.sourceId,
        c.emulation?.platform,
        c.emulation?.gamePath,
      ])
      .sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))),
    Object.entries(game.providerIds ?? {}).filter(([, value]) => value !== undefined && value !== null).sort(([a], [b]) => a.localeCompare(b)),
  ]);
}
export function isEmulated(game: Game): boolean {
  return game.copies.some((c) => c.source === "emulated" || !!c.emulation);
}
export function selectedArtwork(
  game: Game,
  slot: "cover" | "hero",
): ArtworkChoice | undefined {
  const choice = game.artwork?.[slot];
  return choice?.identity === artworkIdentity(game) ? choice : undefined;
}
export function needsGridArtwork(game: Game): boolean {
  return (
    !game.metadataLocked &&
    !isEmulated(game) &&
    !game.cover?.trim() &&
    !selectedArtwork(game, "cover")
  );
}
