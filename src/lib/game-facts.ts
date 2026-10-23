import type { Game } from "./types";
import { currentResult } from "./enrichment-identity";
export const DAY = 86_400_000;
export const GIB = 1024 ** 3;
export function mainExtraHours(game: Game): number | undefined {
  const hours = currentResult(game, "hltb")?.data?.extraHours;
  return typeof hours === "number" && Number.isFinite(hours) && hours > 0
    ? hours
    : undefined;
}
// The largest reported installed copy represents the game's footprint; no directory traversal.
export function installedBytes(game: Game): number | undefined {
  let bytes: number | undefined;
  for (const copy of game.copies)
    if (
      copy.installed &&
      typeof copy.installSizeBytes === "number" &&
      Number.isSafeInteger(copy.installSizeBytes) &&
      copy.installSizeBytes > 0
    )
      bytes = Math.max(bytes ?? 0, copy.installSizeBytes);
  return bytes;
}
export function releaseSpan(game: Game): [number, number] | undefined {
  if (
    typeof game.releasedAt === "number" &&
    Number.isFinite(game.releasedAt) &&
    game.releasedAt >= Date.UTC(1900, 0, 1) &&
    game.releasedAt < Date.UTC(2201, 0, 1)
  )
    return [game.releasedAt, game.releasedAt];
  return game.year &&
    Number.isInteger(game.year) &&
    game.year >= 1900 &&
    game.year <= 2200
    ? [Date.UTC(game.year, 0, 1), Date.UTC(game.year + 1, 0, 1) - 1]
    : undefined;
}
export function calendarDay(text: string | undefined): number | undefined {
  if (!text || !/^\d{4}-\d{2}-\d{2}$/.test(text)) return undefined;
  if (Number(text.slice(0, 4)) < 1900 || Number(text.slice(0, 4)) > 2200)
    return undefined;
  const time = Date.parse(`${text}T00:00:00Z`);
  return Number.isFinite(time) &&
    new Date(time).toISOString().slice(0, 10) === text
    ? time
    : undefined;
}
