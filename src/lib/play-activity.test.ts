import { it, expect } from "vitest";
import { demoGames } from "./demo";
import {
  recentSortAt,
  lastPlayedAt,
  summarizeActivity,
  mergeSteamActivity,
  applySteamHistory,
  type SteamActivity,
} from "./play-activity";
import { mergeOwnedLibrary, filterGames } from "./library";
import { buildHomeShelves } from "./home";
import { versionLibrary } from "./versions";
import { defaultSettings, type Game } from "./types";
const now = Date.UTC(2026, 9, 24),
  account = "test-account";
const fixture = (): Game => ({
  ...demoGames()[0],
  id: "steam:100",
  lastPlayed: now - 50000,
  copies: [
    {
      id: "steam:100",
      source: "steam",
      sourceId: "100",
      installed: true,
      ownership: { accountId: account, verifiedAt: now },
    },
  ],
  launcherActivity: {
    lastLaunchedAt: now - 10000,
    launchCount: 2,
    copyId: "steam:100",
  },
});
const snapshot = (changes: Partial<SteamActivity> = {}): SteamActivity => ({
  copyId: "steam:100",
  appId: 100,
  accountId: account,
  checkedAt: now,
  lastPlayedAt: now - 1000,
  totalMinutes: 120,
  recentMinutes: 30,
  recentCheckedAt: now,
  ...changes,
});
it("keeps imported Steam and local history separate and sorts by the latest requested source", () => {
  const game = { ...fixture(), steamActivity: [snapshot()] };
  expect(lastPlayedAt(game)).toBe(now - 1000);
  expect(lastPlayedAt(game, "launcher")).toBe(now - 10000);
  expect(lastPlayedAt(game, "steam")).toBe(now - 1000);
  const applied = applySteamHistory(
    [game],
    [
      {
        gameId: game.id,
        activities: [snapshot({ checkedAt: now + 1, totalMinutes: 180 })],
      },
    ],
  )[0];
  expect(applied.launcherActivity).toEqual(game.launcherActivity);
  expect(applied.lastPlayed).toBe(game.lastPlayed);
  expect(applied.steamActivity![0].totalMinutes).toBe(180);
  expect(summarizeActivity([game])?.launcherLaunchCount).toBe(2);
});
it("retains unavailable fields, accepts explicit recent zeroes, and rejects stale or rematched/account-mismatched results", () => {
  const game = { ...fixture(), steamActivity: [snapshot()] };
  const merged = mergeSteamActivity(game, [
    snapshot({
      checkedAt: now + 1,
      lastPlayedAt: undefined,
      totalMinutes: undefined,
      recentMinutes: undefined,
      recentCheckedAt: undefined,
    }),
  ])!;
  expect(merged[0]).toMatchObject({
    lastPlayedAt: now - 1000,
    totalMinutes: 120,
    recentMinutes: 30,
    recentCheckedAt: now,
  });
  expect(
    mergeSteamActivity(game, [
      snapshot({ checkedAt: now - 1, totalMinutes: 0 }),
    ])![0].totalMinutes,
  ).toBe(120);
  expect(
    mergeSteamActivity(game, [
      snapshot({
        checkedAt: now + 1,
        recentCheckedAt: now + 1,
        recentMinutes: 0,
      }),
    ])![0].recentMinutes,
  ).toBe(0);
  const changed = { ...game, copies: [{ ...game.copies[0], sourceId: "999" }] };
  expect(
    applySteamHistory(
      [changed],
      [{ gameId: game.id, activities: [snapshot()] }],
    )[0].steamActivity,
  ).toBeUndefined();
  expect(
    mergeSteamActivity(
      {
        ...game,
        copies: [
          {
            ...game.copies[0],
            ownership: { accountId: "another-account", verifiedAt: now },
          },
        ],
      },
      [snapshot()],
    ),
  ).toBeUndefined();
  expect(
    applySteamHistory([], [{ gameId: game.id, activities: [snapshot()] }]),
  ).toEqual([]);
});
it("does not invent an exact last-played date for Steam's recent-only response", () => {
  const game = {
    ...fixture(),
    lastPlayed: undefined,
    launcherActivity: undefined,
    steamActivity: [snapshot({ lastPlayedAt: undefined })],
  };
  expect(lastPlayedAt(game, "steam")).toBeUndefined();
  expect(recentSortAt(game, "steam")).toBe(now - 14 * 86400000);
  expect(
    buildHomeShelves([game], [], now)
      .find((s) => s.id === "played")!
      .games.map((g) => g.id),
  ).toEqual([game.id]);
});
it("keeps legacy local activity when a rematch invalidates Steam history", () => {
  const original = {
    ...fixture(),
    launcherActivity: undefined,
    steamActivity: [snapshot({ lastPlayedAt: undefined })],
  };
  const rematched = {
    ...original,
    copies: [{ ...original.copies[0], sourceId: "101" }],
  };
  const unplayed = {
    ...rematched,
    id: "steam:102",
    lastPlayed: undefined,
    steamActivity: [],
  };
  expect(recentSortAt(original, "steam")).toBe(now - 14 * 86400000);
  expect(recentSortAt(rematched, "steam")).toBeUndefined();
  expect(recentSortAt(rematched, "launcher")).toBe(original.lastPlayed);
  expect(recentSortAt({ ...rematched, steamActivity: [] })).toBe(
    original.lastPlayed,
  );
  expect(recentSortAt(unplayed)).toBeUndefined();
});
it("keeps unchanged history stable and updates valid late copies without reviving stale Steam data", () => {
  const existing = { ...fixture(), steamActivity: [snapshot()] };
  const games = [existing];
  expect(applySteamHistory(games, [])).toBe(games);
  const late = {
    ...fixture(),
    id: "steam:102",
    copies: [{ ...existing.copies[0], id: "steam:102", sourceId: "102" }],
  };
  const update = {
    gameId: late.id,
    activities: [snapshot({ copyId: "steam:102", appId: 102 })],
  };
  expect(applySteamHistory(games, [update])).toBe(games);
  const added = applySteamHistory([...games, late], [update]);
  expect(added[0]).toBe(existing);
  expect(summarizeActivity([added[1]])?.steam).toEqual(update.activities[0]);
  expect(applySteamHistory(added, [update])).toBe(added);
  const rematched = {
    ...added[1],
    copies: [{ ...late.copies[0], sourceId: "103" }],
  };
  expect(summarizeActivity([rematched])?.steam).toBeUndefined();
  const cleaned = applySteamHistory([existing, rematched], [update]);
  expect(cleaned[1].steamActivity).toBeUndefined();
  expect(cleaned[1].launcherActivity).toBe(late.launcherActivity);
});
it("source imports update Steam statistics without replacing local history or carrying another account's snapshot", () => {
  const original = { ...fixture(), steamActivity: [snapshot()] };
  const incoming = {
    ...fixture(),
    lastPlayed: undefined,
    launcherActivity: undefined,
    steamActivity: [
      snapshot({
        checkedAt: now + 1,
        totalMinutes: 300,
        recentMinutes: undefined,
        recentCheckedAt: undefined,
      }),
    ],
  };
  const merged = mergeOwnedLibrary([original], [incoming], "steam")[0];
  expect(merged.launcherActivity).toEqual(original.launcherActivity);
  expect(merged.lastPlayed).toBe(original.lastPlayed);
  expect(merged.steamActivity![0]).toMatchObject({
    totalMinutes: 300,
    recentMinutes: 30,
  });
  const newAccount = {
    ...incoming,
    steamActivity: undefined,
    copies: [
      {
        ...incoming.copies[0],
        ownership: { accountId: "another-account", verifiedAt: now + 1 },
      },
    ],
  };
  expect(
    mergeOwnedLibrary([original], [newAccount], "steam")[0].steamActivity,
  ).toBeUndefined();
});
it("shares duplicate history without double-counting Steam playtime and permits source-specific recent views", () => {
  const steam = {
    ...fixture(),
    lastPlayed: undefined,
    launcherActivity: undefined,
    steamActivity: [snapshot()],
  };
  const local = {
    ...fixture(),
    id: "gog:200",
    lastPlayed: undefined,
    copies: [
      {
        id: "gog:200",
        source: "gog" as const,
        sourceId: "200",
        installed: true,
      },
    ],
    steamActivity: undefined,
    metadataId: 100,
  };
  const index = versionLibrary([steam, local], {
    ...defaultSettings,
    hideDuplicateGames: true,
  });
  const shown = index.withPresentation(local);
  expect(summarizeActivity([steam, local])?.steam?.totalMinutes).toBe(120);
  expect(lastPlayedAt(shown, "steam")).toBe(now - 1000);
  expect(
    filterGames(
      [steam, local],
      "recent",
      "",
      false,
      "",
      "recent",
      [],
      false,
      "",
      "steam",
    ).map((g) => g.id),
  ).toEqual([steam.id]);
  expect(
    filterGames(
      [steam, local],
      "recent",
      "",
      false,
      "",
      "recent",
      [],
      false,
      "",
      "launcher",
    ).map((g) => g.id),
  ).toEqual([local.id]);
});
