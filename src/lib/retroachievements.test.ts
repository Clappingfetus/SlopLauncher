import { expect, it } from "vitest";
import { retroGameId } from "./retroachievements";
import { demoGames } from "./demo";
import {
  identity,
  currentResult,
  resourceLinks,
  EnrichmentQueue,
} from "./enrichment";
import { createGapPolicy } from "./gaps";
import type { Game, ProviderResult } from "./types";
const game = (): Game => ({
  ...demoGames()[0],
  id: "emulated:test",
  metadataId: undefined,
  copies: [
    {
      id: "rom",
      source: "emulated",
      sourceId: "rom",
      installed: true,
      emulation: {
        platform: "ps2",
        profileId: "pcsx2",
        gamePath: "D:/ROMs/Test.chd",
        retroAchievementsGameId: 42,
      },
    },
  ],
});
const tick = async () => {
  for (let i = 0; i < 80; i++) await Promise.resolve();
};
it("accepts only numeric game IDs and exact RetroAchievements game URLs", () => {
  expect(retroGameId(" https://retroachievements.org/game/42 ")).toBe(42);
  expect(retroGameId("42")).toBe(42);
  for (const value of [
    "0",
    "-1",
    "1.5",
    "4294967296",
    "https://evil.com/game/42",
    "https://retroachievements.org.evil.com/game/42",
    "https://retroachievements.org@evil.com/game/42",
    "javascript:42",
  ]) {
    expect(retroGameId(value)).toBeUndefined();
  }
});
it("binds saved progress and resource links to the current account, ROM, console and selected set", () => {
  const g = game();
  const record = {
    status: "ready" as const,
    checkedAt: 10,
    nextCheckAt: 1000,
    fingerprint: identity(g, "retroachievements", 7),
    data: {
      achievements: {
        source: "retroachievements" as const,
        appId: 42,
        accountId: "u",
        progressAvailable: true,
        items: [],
      },
    },
  };
  g.enrichment = { retroachievements: record };
  expect(resourceLinks(g, 7).retroAchievements).toBe(
    "https://retroachievements.org/game/42",
  );
  expect(resourceLinks(g, 8).retroAchievements).toBeUndefined();
  for (const changed of [
    { gamePath: "D:/new.chd" },
    { platform: "ps1" as const },
    { retroAchievementsGameId: 43 },
  ]) {
    const updated = {
      ...g,
      copies: [
        {
          ...g.copies[0],
          emulation: { ...g.copies[0].emulation!, ...changed },
        },
      ],
    };
    expect(currentResult(updated, "retroachievements", 7)).toBeUndefined();
    expect(resourceLinks(updated, 7).retroAchievements).toBeUndefined();
  }
  expect(
    identity(
      { ...g, notes: "Personal", favorite: !g.favorite },
      "retroachievements",
      7,
    ),
  ).toBe(record.fingerprint);
});
it("does not enqueue achievement requests as artwork gaps", () => {
  expect(
    createGapPolicy()(game(), "retroachievements", {
      revision: 7,
      igdbConfigured: true,
      steamGridDbConfigured: true,
      retroAchievementsConfigured: true,
    }),
  ).toBe(false);
});
it("handles late additions and discards old responses after a set change, file change, removal or disconnect", async () => {
  for (const change of ["set", "file", "removed", "account"]) {
    let games: Game[] = [],
      revision = 7,
      resolve!: (v: ProviderResult) => void;
    let applied = 0;
    const queue = new EnrichmentQueue({
      getGames: () => games,
      policy: () => String(revision),
      revisionFor: () => revision,
      now: () => 1,
      enabled: (p) => p === "retroachievements",
      fetch: () => new Promise((r) => (resolve = r)),
      apply: () => applied++,
    });
    queue.reconcile(games);
    games = [game()];
    queue.reconcile(games);
    await tick();
    expect(resolve).toBeDefined();
    if (change === "set")
      games = [
        {
          ...games[0],
          copies: [
            {
              ...games[0].copies[0],
              emulation: {
                ...games[0].copies[0].emulation!,
                retroAchievementsGameId: 43,
              },
            },
          ],
        },
      ];
    if (change === "file")
      games = [
        {
          ...games[0],
          copies: [
            {
              ...games[0].copies[0],
              emulation: {
                ...games[0].copies[0].emulation!,
                gamePath: "D:/new.chd",
              },
            },
          ],
        },
      ];
    if (change === "removed") games = [];
    if (change === "account") revision++;
    resolve({ status: "ready", checkedAt: 1, nextCheckAt: 1000 });
    await tick();
    expect(applied).toBe(0);
    queue.clear();
  }
});
