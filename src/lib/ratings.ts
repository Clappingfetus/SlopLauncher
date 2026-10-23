import type { Game, ProviderId, ReviewRating } from "./types";
import { currentResult } from "./enrichment-identity";

export const reviewSources = [
  "steam",
  "gog",
  "metacritic",
  "opencritic",
  "itch",
] as const;
export const reviewNames = {
  steam: "Steam",
  gog: "GOG",
  metacritic: "Metacritic",
  opencritic: "OpenCritic",
  itch: "itch.io",
};
export type RatingRow = {
  source: ReviewRating["source"];
  rating?: ReviewRating;
  checkedAt?: number;
  stale?: boolean;
  message?: string;
  status?: string;
};
const recordCache = new WeakMap<
  Game,
  Map<string, ReturnType<typeof currentResult>>
>();
function recordsFor(game: Game) {
  let hit = recordCache.get(game);
  if (!hit) {
    hit = new Map(
      reviewSources.map((source) => [
        source,
        currentResult(game, `reviews-${source}` as ProviderId),
      ]),
    );
    recordCache.set(game, hit);
  }
  return hit;
}
export function ratingRows(games: Game[]): RatingRow[] {
  return reviewSources.map((source) => {
    const records = games
      .map((game) => recordsFor(game).get(source))
      .filter((r) => !!r);
    const valid = records
      .filter((r) => {
        const v = r.data?.rating;
        return (
          v?.source === source &&
          v.kind === (source === "opencritic" ? "critics" : "users") &&
          v.scale > 0 &&
          Number.isFinite(v.scale) &&
          Number.isFinite(v.value) &&
          v.value >= 0 &&
          v.value <= v.scale &&
          Number.isSafeInteger(v.count) &&
          v.count > 0
        );
      })
      .sort(
        (a, b) =>
          (b.dataCheckedAt ?? b.checkedAt) - (a.dataCheckedAt ?? a.checkedAt),
      );
    const record =
      valid[0] ?? records.sort((a, b) => b.checkedAt - a.checkedAt)[0];
    return {
      source,
      rating: valid[0]?.data?.rating,
      checkedAt: record?.dataCheckedAt ?? record?.checkedAt,
      stale: record?.status === "unavailable",
      message: record?.message,
      status: record?.status,
    };
  });
}
export function consensus(rows: RatingRow[]) {
  const scores = rows.flatMap((r) => (r.rating ? [r.rating] : []));
  const users = scores.filter((r) => r.kind === "users"),
    critics = scores.filter((r) => r.kind === "critics");
  const average = (items: ReviewRating[]) =>
    items.reduce((sum, r) => sum + (r.value / r.scale) * 10, 0) / items.length;
  if (!scores.length) return undefined;
  const userScore = users.length ? average(users) : undefined;
  const criticScore = critics.length ? average(critics) : undefined;
  const raw =
    userScore !== undefined && criticScore !== undefined
      ? userScore * 0.6 + criticScore * 0.4
      : (userScore ?? criticScore)!;
  // The requested display range is 1–10; preserve precision for ranking within it.
  const score = Math.max(1, Math.min(10, raw));
  return {
    score,
    stars: Math.round(score * 2) / 2,
    userScore,
    criticScore,
    sources: scores.length,
    coverage:
      userScore === undefined
        ? "Critic ratings only"
        : criticScore === undefined
          ? "User ratings only"
          : "60% users · 40% critics",
  };
}
export function consensusScore(game: Game) {
  return consensus(game.reviewSummary ?? ratingRows([game]))?.score;
}
