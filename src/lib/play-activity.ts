import type { Game, Copy } from "./types";
export type ActivitySource = "all" | "steam" | "launcher";
export interface SteamActivity {
  copyId: string;
  appId: number;
  accountId: string;
  checkedAt: number;
  lastPlayedAt?: number;
  totalMinutes?: number;
  recentMinutes?: number;
  recentCheckedAt?: number;
}
export interface LauncherActivity {
  lastLaunchedAt: number;
  launchCount: number;
  copyId: string;
}
export interface ActivitySummary {
  launcherLastAt?: number;
  launcherLaunchCount: number;
  steam?: SteamActivity;
}
export interface HistoryPatch {
  gameId: string;
  activities: SteamActivity[];
}
export interface HistorySync {
  accountId: string;
  revision: number;
  updates: HistoryPatch[];
  nextCheckAt: number;
  warning?: string;
}
export interface LaunchResult {
  message: string;
  launcherActivity?: LauncherActivity;
  historyWarning?: string;
}
const positive = (value: number | undefined) =>
  typeof value === "number" && Number.isFinite(value) && value > 0
    ? value
    : undefined;
export function validSteamActivity(activity: SteamActivity, copies: Copy[]) {
  return (
    Number.isSafeInteger(activity.appId) &&
    activity.appId > 0 &&
    !!activity.accountId &&
    !!positive(activity.checkedAt) &&
    copies.some(
      (c) =>
        c.source === "steam" &&
        c.id === activity.copyId &&
        c.sourceId === String(activity.appId) &&
        (!c.ownership || c.ownership.accountId === activity.accountId),
    )
  );
}
export function mergeSteamActivity(
  game: Game,
  incoming: SteamActivity[],
): SteamActivity[] | undefined {
  const records = new Map(
    (game.steamActivity ?? [])
      .filter((a) => validSteamActivity(a, game.copies))
      .map((a) => [a.copyId, a]),
  );
  for (const next of incoming.filter((a) =>
    validSteamActivity(a, game.copies),
  )) {
    const old = records.get(next.copyId);
    if (old && old.accountId === next.accountId && old.appId === next.appId) {
      if (old.checkedAt > next.checkedAt) continue;
      records.set(next.copyId, {
        ...next,
        lastPlayedAt:
          Math.max(old.lastPlayedAt ?? 0, next.lastPlayedAt ?? 0) || undefined,
        totalMinutes: next.totalMinutes ?? old.totalMinutes,
        ...((next.recentCheckedAt ?? 0) < (old.recentCheckedAt ?? 0)
          ? {
              recentMinutes: old.recentMinutes,
              recentCheckedAt: old.recentCheckedAt,
            }
          : {}),
      });
    } else records.set(next.copyId, next);
  }
  return records.size ? [...records.values()] : undefined;
}
function localPlayedAt(game: Game) {
  return Math.max(
    positive(game.lastPlayed) ?? 0,
    positive(game.launcherActivity?.lastLaunchedAt) ?? 0,
  );
}
export function summarizeActivity(games: Game[]): ActivitySummary | undefined {
  const onlyGame = games[0];
  if (games.length === 1 && (onlyGame.steamActivity?.length ?? 0) <= 1) {
    const local = localPlayedAt(onlyGame);
    const count = Math.max(0, onlyGame.launcherActivity?.launchCount ?? 0);
    const snapshot = onlyGame.steamActivity?.[0];
    const steam =
      snapshot && validSteamActivity(snapshot, onlyGame.copies)
        ? snapshot
        : undefined;
    return local || count || steam
      ? {
          launcherLastAt: local || undefined,
          launcherLaunchCount: count,
          steam,
        }
      : undefined;
  }
  let local = 0,
    count = 0;
  const seen = new Set<string>();
  const steam = new Map<string, SteamActivity>();
  for (const game of games) {
    if (!seen.has(game.id)) {
      seen.add(game.id);
      local = Math.max(local, localPlayedAt(game));
      count += Math.max(0, game.launcherActivity?.launchCount ?? 0);
    }
    for (const item of game.steamActivity ?? []) {
      if (!validSteamActivity(item, game.copies)) continue;
      const key = `${item.accountId}:${item.appId}`,
        old = steam.get(key);
      if (!old || item.checkedAt > old.checkedAt) steam.set(key, item);
    }
  }
  const snapshots = [...steam.values()].sort(
    (a, b) =>
      (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0) ||
      b.checkedAt - a.checkedAt,
  );
  return local || count || snapshots.length
    ? {
        launcherLastAt: local || undefined,
        launcherLaunchCount: count,
        steam: snapshots[0],
      }
    : undefined;
}
export function activity(game: Game) {
  return game.activitySummary ?? summarizeActivity([game]);
}
function exactPlayedAt(
  summary: ActivitySummary | undefined,
  source: ActivitySource,
) {
  const local = source === "steam" ? 0 : (positive(summary?.launcherLastAt) ?? 0),
    steam =
      source === "launcher" ? 0 : (positive(summary?.steam?.lastPlayedAt) ?? 0);
  return Math.max(local, steam) || undefined;
}
export function lastPlayedAt(game: Game, source: ActivitySource = "all") {
  return exactPlayedAt(activity(game), source);
}
export function recentSortAt(game: Game, source: ActivitySource = "all") {
  const summary = activity(game);
  const exact = exactPlayedAt(summary, source);
  const steam = source === "launcher" ? undefined : summary?.steam;
  // Steam may report a recent game without an exact timestamp. Use only the
  // beginning of its reported two-week window as a conservative sorting bound.
  const bound =
    steam?.recentMinutes && steam.recentMinutes > 0 && steam.recentCheckedAt
      ? steam.recentCheckedAt - 14 * 86400000
      : 0;
  return Math.max(exact ?? 0, bound) || undefined;
}
export function applySteamHistory(games: Game[], patches: HistoryPatch[]) {
  if (!patches.length) return games;
  const byId = new Map(patches.map((p) => [p.gameId, p]));
  let result = games;
  for (let index = 0; index < games.length; index++) {
    const game = games[index];
    const patch = byId.get(game.id);
    if (!patch) continue;
    const steamActivity = mergeSteamActivity(game, patch.activities);
    if (JSON.stringify(steamActivity) === JSON.stringify(game.steamActivity))
      continue;
    if (result === games) result = games.slice();
    result[index] = { ...game, steamActivity };
  }
  return result;
}
