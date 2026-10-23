import { it, expect } from "vitest";
import { demoGames } from "./demo";
import { EnrichmentQueue, identity, currentResult } from "./enrichment";
import { createGapPolicy, gapProviderEnabled } from "./gaps";
import type { Game, ProviderResult } from "./types";
const tick = async () => {
  for (let i = 0; i < 80; i++) await Promise.resolve();
};
const result: ProviderResult = {
  status: "ready",
  checkedAt: 100,
  nextCheckAt: 1000,
  data: {
    achievements: {
      source: "steam",
      appId: 42,
      accountId: "fixture",
      progressAvailable: true,
      items: [],
    },
  },
};
it("does not add achievements to startup gap searches", () => {
  const game = { ...demoGames()[0], cover: undefined };
  const status = {
    revision: 0,
    steamGridDbConfigured: false,
    igdbConfigured: false,
  };
  expect(gapProviderEnabled(game, "achievements", status)).toBe(false);
  expect(createGapPolicy()(game, "achievements", status)).toBe(false);
});
it("binds progress to current identity, copy, and account while preserving personal changes", () => {
  const game = demoGames()[0],
    key = identity(game, "achievements", 10);
  expect(
    identity(
      { ...game, notes: "New note", favorite: !game.favorite },
      "achievements",
      10,
    ),
  ).toBe(key);
  for (const changed of [
    { ...game, title: "Renamed" },
    { ...game, metadataId: 77 },
    { ...game, copies: [{ ...game.copies[0], sourceId: "77" }] },
  ])
    expect(identity(changed, "achievements", 10)).not.toBe(key);
  expect(identity(game, "achievements", 11)).not.toBe(key);
  const cached = {
    ...game,
    enrichment: { achievements: { ...result, fingerprint: key } },
  };
  expect(currentResult(cached, "achievements", 10)).toBeDefined();
  expect(currentResult(cached, "achievements", 11)).toBeUndefined();
});
it("loads late additions only on demand and discards removed, rematched, and old account responses", async () => {
  for (const change of ["removed", "rematched", "account", "copy"]) {
    let games: Game[] = [],
      revision = 10,
      selected = "",
      resolve!: (value: ProviderResult) => void;
    const applied: string[] = [];
    const queue = new EnrichmentQueue({
      getGames: () => games,
      policy: () => String(revision) + selected,
      revisionFor: () => revision,
      now: () => 100,
      enabled: (p, g) => p === "achievements" && g.id === selected,
      fetch: () => new Promise((r) => (resolve = r)),
      apply: (id) => applied.push(id),
    });
    queue.reconcile(games);
    games = [demoGames()[0]];
    queue.reconcile(games);
    await tick();
    expect(resolve).toBeUndefined();
    selected = games[0].id;
    queue.reconcile(games);
    await tick();
    expect(resolve).toBeDefined();
    if (change === "removed") games = [];
    if (change === "rematched") games = [{ ...games[0], metadataId: 77 }];
    if (change === "account") revision++;
    if (change === "copy")
      games = [
        { ...games[0], copies: [{ ...games[0].copies[0], sourceId: "77" }] },
      ];
    resolve(result);
    await tick();
    expect(applied).toEqual([]);
    queue.clear();
  }
});
it("keeps cached progress on a transient failure without overwriting notes", async () => {
  let games = [
    {
      ...demoGames()[0],
      notes: "Keep this",
      enrichment: {
        achievements: {
          ...result,
          nextCheckAt: 1,
          fingerprint: identity(demoGames()[0], "achievements", 10),
        },
      },
    },
  ];
  const queue = new EnrichmentQueue({
    getGames: () => games,
    revisionFor: () => 10,
    now: () => 200,
    enabled: (p) => p === "achievements",
    fetch: async () => {
      throw Error("offline");
    },
    apply: (id, p, value) => {
      games = games.map((g) =>
        g.id === id ? { ...g, enrichment: { ...g.enrichment, [p]: value } } : g,
      );
    },
  });
  queue.reconcile(games);
  await tick();
  expect(games[0].enrichment.achievements.data).toEqual(result.data);
  expect(games[0].enrichment.achievements.status).toBe("unavailable");
  expect(games[0].notes).toBe("Keep this");
  queue.clear();
});
