import { describe, it, expect } from "vitest";
import {
  gapProviderEnabled,
  createGapPolicy,
  missingFields,
  nativeProvider,
  gapProviderOrder,
} from "./gaps";
import {
  EnrichmentQueue,
  identity,
  presentGame,
  type EnrichmentProgress,
} from "./enrichment";
import type { Game, ProviderId, ProviderResult } from "./types";
const status = {
  revision: 7,
  steamGridDbConfigured: true,
  igdbConfigured: true,
  screenScraperConfigured: true,
};
const complete = (): Game => ({
  id: "pc",
  title: "PC game",
  copies: [
    { id: "steam:42", source: "steam", sourceId: "42", installed: true },
  ],
  cover: "native",
  hero: "wide",
  description: "Story",
  developer: "Studio",
  publisher: "Publisher",
  year: 2026,
  genres: ["Action"],
  screenshots: ["shot"],
  notes: "Personal",
  favorite: true,
  status: "playing",
  addedAt: 1,
});
const result = (g: Game, p: ProviderId, data: object): void => {
  g.enrichment = {
    ...g.enrichment,
    [p]: {
      status: "ready",
      checkedAt: 1,
      nextCheckAt: 900000,
      fingerprint: identity(g, p, status.revision),
      data,
    },
  };
};
const tick = async () => {
  for (let i = 0; i < 80; i++) await Promise.resolve();
};
describe("automatic gap search", () => {
  it("starts different metadata sources without letting one rate limit occupy every worker", () => {
    const first = complete();
    first.copies.push(
      { id: "gog:99", source: "gog", sourceId: "99", installed: false },
      {
        id: "emu",
        source: "emulated",
        sourceId: "emu",
        installed: true,
        emulation: {
          platform: "ps1",
          profileId: "ps1",
          gamePath: "D:\\game.chd",
        },
      },
    );
    const games = [first, { ...first, id: "second" }];
    const called: ProviderId[] = [];
    const queue = new EnrichmentQueue({
      getGames: () => games,
      concurrency: 4,
      order: (p) => gapProviderOrder[p],
      enabled: (p) => ["steam", "stores", "screenscraper", "igdb"].includes(p),
      fetch: (_g, p) => {
        called.push(p);
        return new Promise(() => {});
      },
      apply: () => {},
    });
    queue.reconcile(games);
    expect(new Set(called).size).toBe(4);
    queue.clear();
  });
  it("skips all complete or locked games instead of refreshing every provider", () => {
    const g = complete();
    expect(missingFields(g)).toEqual([]);
    for (const p of Object.keys(gapProviderOrder) as ProviderId[])
      expect(gapProviderEnabled(g, p, status)).toBe(false);
    g.cover = undefined;
    g.metadataLocked = true;
    expect(missingFields(g)).toEqual([]);
    expect(gapProviderEnabled(g, "steam", status)).toBe(false);
  });
  it("tries a native store before third-party artwork or metadata", () => {
    const g = complete();
    g.cover = undefined;
    g.description = undefined;
    expect(nativeProvider(g)).toBe("steam");
    expect(gapProviderEnabled(g, "steam", status)).toBe(true);
    expect(gapProviderEnabled(g, "igdb", status)).toBe(false);
    expect(gapProviderEnabled(g, "steamgriddb", status)).toBe(false);
    result(g, "steam", { cover: "store-cover", summary: "Store description" });
    expect(missingFields(presentGame(g, 7))).toEqual([]);
    expect(gapProviderEnabled(g, "steamgriddb", status)).toBe(false);
    expect(g.notes).toBe("Personal");
  });
  it("fills only missing fields and retains existing text, store artwork, choices and launch data", () => {
    const g = complete();
    g.publisher = undefined;
    result(g, "igdb", {
      summary: "Replacement",
      cover: "replacement",
      publisher: "Filled",
    });
    const shown = presentGame(g, 7);
    expect(shown.description).toBe("Story");
    expect(shown.cover).toBe("native");
    expect(shown.publisher).toBe("Filled");
    expect(shown.copies).toEqual(g.copies);
  });
  it("can fall back after a native failure but never applies SteamGridDB to an emulator automatically", () => {
    const g = complete();
    g.cover = undefined;
    result(g, "steam", {});
    g.enrichment!.steam!.status = "unavailable";
    expect(gapProviderEnabled(g, "steamgriddb", status)).toBe(true);
    g.copies = [
      {
        id: "emu",
        source: "emulated",
        sourceId: "emu",
        installed: true,
        emulation: {
          platform: "ps1",
          profileId: "ps1",
          gamePath: "D:\\Games\\future.chd",
        },
      },
    ];
    expect(nativeProvider(g)).toBe("screenscraper");
    expect(gapProviderEnabled(g, "steamgriddb", status)).toBe(false);
    expect(gapProviderEnabled(g, "screenscraper", status)).toBe(true);
  });
  it("reports bounded parallel work and immediately clears progress when cancelled", async () => {
    let games = [complete(), { ...complete(), id: "late", title: "Late game" }];
    const progress: EnrichmentProgress[] = [];
    const releases: ((r: ProviderResult) => void)[] = [];
    const queue = new EnrichmentQueue({
      getGames: () => games,
      concurrency: 2,
      enabled: (p) => p === "steam",
      onProgress: (p) => progress.push(p),
      fetch: () => new Promise((resolve) => releases.push(resolve)),
      apply: () => {},
    });
    queue.reconcile(games, 7);
    expect(progress.at(-1)).toMatchObject({
      running: 2,
      queued: 0,
      total: 2,
      completed: 0,
    });
    queue.clear();
    expect(progress.at(-1)).toMatchObject({ running: 0, total: 0 });
    releases.forEach((release) =>
      release({ status: "ready", checkedAt: 1, nextCheckAt: 100, data: {} }),
    );
    await tick();
    expect(progress.at(-1)?.running).toBe(0);
  });
  it("handles late arrivals, disable/re-enable, renames, removal and retry backoff", async () => {
    let games: Game[] = [];
    let enabled = true;
    let now = 10;
    const called: string[] = [];
    const progress: EnrichmentProgress[] = [];
    const queue = new EnrichmentQueue({
      getGames: () => games,
      now: () => now,
      enabled: (p, g) =>
        enabled && p === "igdb" && gapProviderEnabled(g, p, status),
      onProgress: (p) => progress.push(p),
      fetch: async (g) => {
        called.push(g.title);
        return {
          status: "unavailable",
          checkedAt: now,
          nextCheckAt: now + 100,
        };
      },
      apply: (id, p, value) => {
        games = games.map((g) =>
          g.id === id
            ? { ...g, enrichment: { ...g.enrichment, [p]: value } }
            : g,
        );
      },
    });
    queue.reconcile(games, 7);
    games = [
      {
        ...complete(),
        copies: [{ id: "m", source: "manual", sourceId: "m", installed: true }],
        cover: undefined,
      },
    ];
    queue.reconcile(games, 7);
    await tick();
    expect(called).toEqual(["PC game"]);
    queue.reconcile(games, 7);
    await tick();
    expect(called).toHaveLength(1);
    enabled = false;
    now = 200;
    queue.reconcile(games, 7);
    await tick();
    expect(called).toHaveLength(1);
    enabled = true;
    queue.reconcile(games, 7);
    await tick();
    expect(called).toHaveLength(2);
    games = [{ ...games[0], title: "Rematched" }];
    queue.reconcile(games, 7);
    await tick();
    expect(called.at(-1)).toBe("Rematched");
    games = [];
    queue.reconcile(games, 7);
    expect(progress.at(-1)?.running).toBe(0);
    queue.clear();
  });
});

it("cached gap policies agree with provider eligibility and invalidate with new records or credentials", () => {
  const policy = createGapPolicy(),
    g = {
      ...complete(),
      cover: undefined,
      hero: undefined,
      description: undefined,
    };
  for (const p of Object.keys(gapProviderOrder) as ProviderId[])
    expect(policy(g, p, status)).toBe(gapProviderEnabled(g, p, status));
  const next = {
    ...g,
    enrichment: {
      steam: {
        status: "ready" as const,
        checkedAt: 1,
        nextCheckAt: 2,
        fingerprint: identity(g, "steam", 7),
        data: { cover: "store", hero: "wide", summary: "Story" },
      },
    },
  };
  for (const p of Object.keys(gapProviderOrder) as ProviderId[])
    expect(policy(next, p, status)).toBe(gapProviderEnabled(next, p, status));
  expect(policy(g, "igdb", { ...status, igdbConfigured: false })).toBe(false);
});
