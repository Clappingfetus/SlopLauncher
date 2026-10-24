import type { Game } from "./types";
export function exactReleaseDate(value: unknown): number | undefined {
  return typeof value === "number" &&
    Number.isSafeInteger(value) &&
    value >= Date.UTC(1900, 0, 1) &&
    value < Date.UTC(2201, 0, 1)
    ? value
    : undefined;
}
// Provider dates are calendar days, not local instants: UTC formatting prevents
// a midnight release from appearing a day early in western time zones.
export function releaseLabel(game: Pick<Game, "releasedAt" | "year">): string {
  const at = exactReleaseDate(game.releasedAt);
  return at !== undefined
    ? new Intl.DateTimeFormat(undefined, {
        year: "numeric",
        month: "short",
        day: "numeric",
        timeZone: "UTC",
      }).format(at)
    : game.year
      ? String(game.year)
      : "Not available";
}
