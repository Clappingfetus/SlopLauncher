import { describe, it, expect, vi } from "vitest";
import {
  EnrichmentQueue,
  createGamePresenter,
  identity,
  resourceLinks,
  presentGame,
  currentResult,
  hours,
} from "./enrichment";
import { demoGames } from "./demo";
import {
  providerIds,
  type Game,
  type ProviderId,
  type ProviderResult,
  type EnrichmentRecord,
} from "./types";
const DAY = 86_400_000;
const tick = async () => {
  for (let i = 0; i < 80; i++) await Promise.resolve();
};
const fixture = (id: string, title: string): Game => ({
  ...demoGames()[0],
  id,
  title,
  metadataId: undefined,
  copies: [{ id, source: "manual", sourceId: id, installed: true }],
  enrichment: undefined,
});
const success = (now: number, data = { hltbId: 7 }): ProviderResult => ({
  status: "ready",
  checkedAt: now,
  nextCheckAt: now + DAY,
  data,
});
function harness(initial: Game[]) {
  let games = initial,
    time = 1000;
  const applied: {
    id: string;
    provider: ProviderId;
    value: EnrichmentRecord;
  }[] = [];
  const fetch = vi.fn(async (_g: Game, _p: ProviderId) => success(time));
  const queue = new EnrichmentQueue({
    getGames: () => games,
    now: () => time,
    fetch: (g, p) => fetch(g, p),
    apply: (id, provider, value) => {
      applied.push({ id, provider, value });
      games = games.map((g) =>
        g.id === id
          ? { ...g, enrichment: { ...g.enrichment, [provider]: value } }
          : g,
      );
    },
  });
  return {
    queue,
    fetch,
    applied,
    get games() {
      return games;
    },
    set games(value) {
      games = value;
    },
    advance(ms: number) {
      time += ms;
    },
    get time() {
      return time;
    },
  };
}
describe("resource links follow a changing library", () => {
  it("runs ScreenScraper for late console additions and keeps PC games out of its queue", async () => {
    const h = harness([fixture("pc", "PC Game")]);
    h.queue.reconcile(h.games);
    await tick();
    expect(h.fetch.mock.calls.some(([, p]) => p === "screenscraper")).toBe(
      false,
    );
    const game = fixture("console", "Future Console Game");
    game.copies = [
      {
        id: "emu",
        source: "emulated",
        sourceId: "emu",
        installed: true,
        emulation: {
          platform: "ps1",
          profileId: "profile",
          gamePath: "D:\\ROMs\\future.chd",
        },
      },
    ];
    h.games = [...h.games, game];
    h.queue.reconcile(h.games, 7);
    await tick();
    expect(
      h.fetch.mock.calls.some(
        ([g, p]) => g.id === "console" && p === "screenscraper",
      ),
    ).toBe(true);
    const current = h.games.find((g) => g.id === "console")!;
    expect(currentResult(current, "screenscraper", 7)?.status).toBe("ready");
    expect(currentResult(current, "screenscraper", 8)).toBeUndefined();
    current.copies[0] = {
      ...current.copies[0],
      emulation: { ...current.copies[0].emulation!, gamePath: "E:\\moved.chd" },
    };
    expect(currentResult(current, "screenscraper", 7)).toBeUndefined();
  });
  it("presents console metadata and genre tags without overriding locked metadata or personal fields", () => {
    const game = fixture("console", "Future Console Game");
    game.description = undefined;
    game.copies = [
      {
        id: "console",
        source: "emulated",
        sourceId: "console",
        installed: true,
      },
    ];
    game.favorite = true;
    game.notes = "My notes";
    game.enrichment = {
      screenscraper: {
        ...success(1),
        fingerprint: identity(game, "screenscraper", 9),
        data: {
          cover: "ss-art://57/45/box-2D/us",
          summary: "Console story",
          developer: "Console studio",
          year: 1998,
          genres: ["Adventure"],
          tags: ["Adventure"],
        },
      },
    };
    const presented = presentGame(game, 9);
    expect(presented.cover).toBe("ss-art://57/45/box-2D/us");
    expect(presented.description).toBe("Console story");
    expect(presented.tags).toContain("Adventure");
    expect(presented.notes).toBe("My notes");
    expect(presented.favorite).toBe(true);
    game.metadataLocked = true;
    expect(presentGame(game, 9).cover).toBe(game.cover);
    expect(presentGame(game, 9).description).toBe(game.description);
  });
  it("enriches itch games added after startup and derives their details links from current identity", async () => {
    const h = harness([]);
    h.queue.reconcile(h.games);
    const game = fixture("itch:42", "Late itch game");
    game.copies = [
      { id: "itch:42", source: "itch", sourceId: "42", installed: false },
    ];
    h.games = [game];
    h.queue.reconcile(h.games);
    await tick();
    expect(h.fetch).toHaveBeenCalled();
    expect(resourceLinks(h.games[0]).hltb).toContain("howlongtobeat.com");
    expect(resourceLinks(h.games[0]).pcgw).toContain("pcgamingwiki.com");
    expect(resourceLinks(h.games[0]).store).toBe(
      "https://itch.io/search?q=Late%20itch%20game",
    );
    h.queue.clear();
  });
  it("uses the owned Steam copy for Discussions, Guides, and Store links", () => {
    const game = fixture("steam-game", "A future Steam release");
    game.copies = [
      {
        id: "steam:123456",
        source: "steam",
        sourceId: "123456",
        installed: false,
      },
    ];
    game.metadataId = 999999;
    expect(resourceLinks(game)).toMatchObject({
      discussions: "https://steamcommunity.com/app/123456/discussions/",
      guides: "https://steamcommunity.com/app/123456/guides/",
      store: "https://store.steampowered.com/app/123456/",
    });
    game.copies[0].sourceId = "543210";
    expect(resourceLinks(game).store).toContain("/543210/");
  });
  it("does not mistake a manual Steam metadata match for a Steam-owned copy", () => {
    const game = fixture("manual", "A manual game");
    game.metadataId = 123456;
    expect(resourceLinks(game).store).toBeUndefined();
    expect(resourceLinks(game).guides).toBeUndefined();
    expect(resourceLinks(game).discussions).toBeUndefined();
  });
  it("shows Nexus only for a current verified catalog match with published mods", () => {
    const game = fixture("new", "A new modded game");
    expect(resourceLinks(game).nexus).toBeUndefined();
    game.enrichment = {
      nexus: {
        ...success(1),
        fingerprint: identity(game, "nexus"),
        data: {
          nexusDomain: "verifiedgame",
          modCount: 12,
          url: "https://evil.test",
        },
      },
    };
    expect(resourceLinks(game).nexus).toBe(
      "https://www.nexusmods.com/verifiedgame/mods/",
    );
    game.enrichment.nexus!.data!.modCount = 0;
    expect(resourceLinks(game).nexus).toBeUndefined();
    game.enrichment.nexus!.data!.modCount = 12;
    game.title = "Different game";
    expect(resourceLinks(game).nexus).toBeUndefined();
  });
  it("rejects invalid Nexus domains even when cached data has a URL", () => {
    const game = fixture("bad", "Example");
    game.enrichment = {
      nexus: {
        ...success(1),
        fingerprint: identity(game, "nexus"),
        data: {
          nexusDomain: "evil.test/path",
          modCount: 10,
          url: "https://evil.test",
        },
      },
    };
    expect(resourceLinks(game).nexus).toBeUndefined();
  });
  it("creates links for arbitrary future games without a scanned mapping", () => {
    const game = fixture("new-id", "An entirely new game & a sequel");
    const links = resourceLinks(game);
    expect(links.pcgw).toContain(encodeURIComponent(game.title));
    expect(links.hltb).toContain(encodeURIComponent(game.title));
    game.title = "Renamed / Other Edition";
    expect(resourceLinks(game).hltb).toContain(encodeURIComponent(game.title));
  });
  it("prefers current store identity and explicit HLTB matches", () => {
    const game = fixture("new", "Example");
    game.metadataId = 987654;
    expect(resourceLinks(game).pcgw).toBe(
      "https://www.pcgamingwiki.com/api/appid.php?appid=987654",
    );
    game.providerIds = { hltb: 12345 };
    expect(resourceLinks(game).hltb).toBe(
      "https://howlongtobeat.com/game/12345",
    );
  });
  it("rejects a stored article after a rename or rematch", () => {
    const game = fixture("one", "Original");
    game.enrichment = {
      pcgw: {
        ...success(1, { hltbId: 5 }),
        fingerprint: identity(game, "pcgw"),
        data: { url: "https://www.pcgamingwiki.com/wiki/Original", hltbId: 5 },
      },
    };
    expect(resourceLinks(game).wikiResolved).toBe(true);
    game.title = "Another game";
    expect(resourceLinks(game).wikiResolved).toBe(false);
    expect(resourceLinks(game).pcgw).not.toContain("/wiki/Original");
    expect(resourceLinks(game).hltb).not.toContain("/game/5");
  });
  it("keeps absent estimates absent", () => {
    expect(hours(undefined)).toBe("—");
    expect(hours(0)).toBe("—");
    expect(hours(1.5)).toContain("1.5");
  });
});
describe("continuous enrichment queue", () => {
  it("imports tags for late games, discards old matches, and retains custom and cached tags on failure", async () => {
    const h = harness([]);
    h.fetch.mockImplementation(async (_g, p) => ({
      ...success(h.time),
      data: p === "steam" ? { tags: ["Automatic"] } : {},
    }));
    h.queue.reconcile(h.games);
    await tick();
    const first = fixture("late", "Late arrival");
    first.tags = ["Mine"];
    h.games = [first];
    h.queue.reconcile(h.games);
    await tick();
    expect(presentGame(h.games[0]).tags).toContain("Automatic");
    h.advance(DAY + 1);
    h.fetch.mockRejectedValue(new Error("Offline"));
    h.queue.reconcile(h.games);
    await tick();
    expect(presentGame(h.games[0]).tags).toEqual(
      expect.arrayContaining(["Mine", "Automatic"]),
    );
    h.games = h.games.map((g) => ({ ...g, title: "Rematched game" }));
    expect(presentGame(h.games[0]).tags).not.toContain("Automatic");
    h.queue.clear();
  });
  it("can resolve Nexus while a slower wiki request is still running", async () => {
    const h = harness([fixture("new", "A newly added game")]);
    let release!: () => void;
    h.fetch.mockImplementation(async (_g, p) => {
      if (p === "pcgw")
        await new Promise<void>((resolve) => (release = resolve));
      return success(h.time);
    });
    h.queue.reconcile(h.games);
    await tick();
    expect(h.games[0].enrichment?.pcgw).toBeUndefined();
    expect(h.games[0].enrichment?.nexus?.status).toBe("ready");
    release();
    await tick();
    h.queue.clear();
  });
  it("enriches additions made after its initial work has completed", async () => {
    const h = harness([fixture("a", "First")]);
    h.queue.reconcile(h.games);
    await tick();
    expect(h.games[0].enrichment?.hltb?.status).toBe("ready");
    h.games = [...h.games, fixture("b", "Added later")];
    h.queue.reconcile(h.games);
    await tick();
    for (const provider of providerIds.filter(
      (p) => p !== "stores" && p !== "screenscraper",
    ))
      expect(
        h.fetch.mock.calls.some(([g, p]) => g.id === "b" && p === provider),
      ).toBe(true);
    expect(h.fetch.mock.calls.filter(([g]) => g.id === "a")).toHaveLength(
      providerIds.length - 2,
    );
    h.queue.clear();
  });
  it("deduplicates active work while the library changes", async () => {
    const h = harness([fixture("a", "First")]);
    const releases: (() => void)[] = [];
    let running = 0,
      max = 0;
    h.fetch.mockImplementation(async () => {
      running++;
      max = Math.max(max, running);
      await new Promise<void>((resolve) => releases.push(resolve));
      running--;
      return success(h.time);
    });
    for (let i = 0; i < 10; i++) h.queue.reconcile(h.games);
    expect(h.fetch).toHaveBeenCalledTimes(2);
    for (let i = 0; i < 8; i++) {
      releases.splice(0).forEach((f) => f());
      await tick();
    }
    expect(max).toBeLessThanOrEqual(2);
    expect(
      new Set(h.fetch.mock.calls.map(([g, p]) => `${g.id}:${p}`)).size,
    ).toBe(h.fetch.mock.calls.length);
    h.queue.clear();
  });
  it("discards in-flight results after the identity changes", async () => {
    const h = harness([fixture("a", "Original")]);
    let release!: () => void;
    h.fetch.mockImplementation(async (g, p) => {
      if (g.title === "Original" && p === "pcgw")
        await new Promise<void>((resolve) => (release = resolve));
      return success(h.time);
    });
    h.queue.reconcile(h.games);
    await tick();
    h.games = h.games.map((g) => ({ ...g, title: "Rematched" }));
    h.queue.reconcile(h.games);
    await tick();
    const before = h.applied.length;
    release();
    await tick();
    expect(
      h.applied
        .slice(before)
        .some(
          (a) =>
            a.provider === "pcgw" && a.value.fingerprint.includes("Original"),
        ),
    ).toBe(false);
    expect(currentResult(h.games[0], "pcgw")?.status).toBe("ready");
    h.queue.clear();
  });
  it("does not resurrect removed games", async () => {
    const h = harness([fixture("a", "First")]);
    let release!: () => void;
    h.fetch.mockImplementation(async (_g, p) => {
      if (p === "pcgw")
        await new Promise<void>((resolve) => (release = resolve));
      return success(h.time);
    });
    h.queue.reconcile(h.games);
    await tick();
    h.games = [];
    h.queue.reconcile([]);
    const before = h.applied.length;
    release();
    await tick();
    expect(h.games).toHaveLength(0);
    expect(h.applied).toHaveLength(before);
    h.queue.clear();
  });
  it("isolates failures, observes backoff, and retries later", async () => {
    const h = harness([fixture("a", "First")]);
    let fail = true;
    h.fetch.mockImplementation(async (_g, p) => {
      if (p === "pcgw" && fail) throw new Error("offline");
      return success(h.time);
    });
    h.queue.reconcile(h.games);
    await tick();
    expect(h.games[0].enrichment?.pcgw?.status).toBe("unavailable");
    expect(h.games[0].enrichment?.steamgriddb?.status).toBe("ready");
    const calls = h.fetch.mock.calls.filter(([, p]) => p === "pcgw").length;
    h.queue.reconcile(h.games);
    await tick();
    expect(h.fetch.mock.calls.filter(([, p]) => p === "pcgw")).toHaveLength(
      calls,
    );
    fail = false;
    h.advance(15 * 60_000 + 1);
    h.queue.reconcile(h.games);
    await tick();
    expect(h.games[0].enrichment?.pcgw?.status).toBe("ready");
    h.queue.clear();
  });
  it("refreshes keyed providers after credentials change without re-fetching public wiki data", async () => {
    const h = harness([fixture("a", "First")]);
    h.queue.reconcile(h.games, 1);
    await tick();
    const before = h.fetch.mock.calls.length;
    h.queue.reconcile(h.games, 2);
    await tick();
    const later = h.fetch.mock.calls.slice(before).map(([, p]) => p);
    expect(later).toEqual(expect.arrayContaining(["igdb", "steamgriddb"]));
    expect(later).not.toContain("pcgw");
    h.queue.clear();
  });
  it("preserves personal changes made during an external request", async () => {
    const h = harness([fixture("a", "First")]);
    h.queue.reconcile(h.games);
    h.games = h.games.map((g) => ({
      ...g,
      notes: "Keep my note",
      favorite: true,
      status: "completed",
    }));
    await tick();
    expect(h.games[0]).toMatchObject({
      notes: "Keep my note",
      favorite: true,
      status: "completed",
    });
    h.queue.clear();
  });
  it("retains cached provider data if refreshing fails", async () => {
    const h = harness([fixture("a", "First")]);
    h.queue.reconcile(h.games);
    await tick();
    const previous = h.games[0].enrichment!.pcgw!;
    h.advance(DAY + 1);
    h.fetch.mockImplementation(async () => {
      throw new Error("offline");
    });
    h.queue.reconcile(h.games);
    await tick();
    expect(h.games[0].enrichment!.pcgw!.status).toBe("unavailable");
    expect(h.games[0].enrichment!.pcgw!.data).toEqual(previous.data);
    h.queue.clear();
  });
  it("never leaks old requests into the sample library", async () => {
    const h = harness([fixture("a", "First")]);
    let release!: () => void;
    h.fetch.mockImplementation(async (_g, p) => {
      if (p === "pcgw")
        await new Promise<void>((resolve) => (release = resolve));
      return success(h.time);
    });
    h.queue.reconcile(h.games);
    await tick();
    h.queue.clear();
    const before = h.applied.length;
    release();
    await tick();
    expect(h.applied).toHaveLength(before);
  });
});
describe("presentation preserves stored data", () => {
  it("honors locked metadata and does not overwrite the stored Steam fields", () => {
    const game = fixture("a", "First");
    game.enrichment = {
      steamgriddb: {
        ...success(1),
        fingerprint: identity(game, "steamgriddb", 4),
        data: { cover: "https://cdn2.steamgriddb.com/grid/example.png" },
      },
    };
    const old = game.cover;
    expect(presentGame(game, 4).cover).toBe(old);
    expect(game.cover).toBe(old);
    game.metadataLocked = true;
    expect(presentGame(game, 4).cover).toBe(old);
  });
});

describe("release date precision for grouping", () => {
  it("keeps a store release year when IGDB describes an earlier platform release", () => {
    const game = fixture("release-date", "Platform port");
    game.year = 2023;
    const releasedAt = Date.UTC(2020, 5, 10);
    game.enrichment = {
      igdb: {
        ...success(1000),
        fingerprint: identity(game, "igdb"),
        data: { year: 2020, releasedAt },
      },
    };
    expect(presentGame(game).releasedAt).toBeUndefined();
    game.enrichment.igdb!.data = {
      year: 2023,
      releasedAt: Date.UTC(2023, 5, 10),
    };
    expect(presentGame(game).releasedAt).toBe(Date.UTC(2023, 5, 10));
    game.releasedAt = Date.UTC(2023, 8, 12);
    expect(presentGame(game).releasedAt).toBe(game.releasedAt);
  });
});

describe("incremental background scheduling", () => {
  it("rechecks the completed game without scanning all other games", async () => {
    let games = Array.from({ length: 1000 }, (_, i) =>
      fixture(String(i), `Game ${i}`),
    );
    let index = new Map(games.map((g) => [g.id, g]));
    let release!: (r: ProviderResult) => void;
    const checks: string[] = [];
    const queue = new EnrichmentQueue({
      getGames: () => games,
      getGame: (id) => index.get(id),
      concurrency: 1,
      policy: () => "same",
      enabled: (p, g) => {
        checks.push(g.id);
        return p === "steam";
      },
      fetch: () => new Promise((r) => (release = r)),
      apply: (id, p, r) => {
        games = games.map((g) =>
          g.id === id ? { ...g, enrichment: { [p]: r } } : g,
        );
        index = new Map(games.map((g) => [g.id, g]));
      },
    });
    queue.reconcile(games);
    checks.length = 0;
    release(success(1));
    await tick();
    expect(new Set(checks).size).toBeLessThan(4);
    expect(checks.length).toBeLessThan(20);
    queue.clear();
  });
  it("bounds each provider so a blocked service cannot occupy every worker", () => {
    const games = Array.from({ length: 20 }, (_, i) =>
        fixture(String(i), `Game ${i}`),
      ),
      called: ProviderId[] = [];
    const queue = new EnrichmentQueue({
      getGames: () => games,
      concurrency: 6,
      perProviderConcurrency: 2,
      enabled: (p) => ["steam", "igdb", "pcgw"].includes(p),
      order: (p) => (p === "steam" ? 0 : p === "igdb" ? 1 : 4),
      fetch: (_g, p) => {
        called.push(p);
        return new Promise(() => {});
      },
      apply: () => {},
    });
    queue.reconcile(games);
    expect(called).toHaveLength(6);
    for (const p of ["steam", "igdb", "pcgw"])
      expect(called.filter((x) => x === p)).toHaveLength(2);
    queue.clear();
  });
  it("yields before startup work, cancels scheduled work, and accepts late games", async () => {
    vi.useFakeTimers();
    try {
      let games = [fixture("one", "First")];
      const fetch = vi.fn(
        (_g: Game, _p: ProviderId) => new Promise<ProviderResult>(() => {}),
      );
      const queue = new EnrichmentQueue({
        getGames: () => games,
        enabled: (p) => p === "steam",
        fetch,
        apply: () => {},
      });
      queue.scheduleReconcile(games);
      expect(fetch).not.toHaveBeenCalled();
      queue.clear();
      await vi.advanceTimersByTimeAsync(100);
      expect(fetch).not.toHaveBeenCalled();
      games = [fixture("late", "Late")];
      queue.scheduleReconcile(games);
      await vi.advanceTimersByTimeAsync(100);
      expect(fetch).toHaveBeenCalledTimes(1);
      expect(fetch.mock.calls[0][0].id).toBe("late");
      queue.clear();
    } finally {
      vi.useRealTimers();
    }
  });
  it("reuses unchanged presentations and updates immutable metadata replacements", () => {
    const show = createGamePresenter(),
      g = fixture("same", "Same");
    expect(show(g)).toBe(show(g));
    const changed = {
      ...g,
      enrichment: {
        steam: {
          ...success(1),
          fingerprint: identity(g, "steam"),
          data: { summary: "New metadata" },
        },
      },
    };
    expect(show(changed).description).toBe(g.description || "New metadata");
    expect(show(changed)).not.toBe(show(g));
    expect(show(g, 2)).not.toBe(show(g, 1));
  });
});

it("starts independent services before doubling up and bounds twelve concurrent lookups", async () => {
  const games = Array.from({ length: 20 }, (_, i) =>
    fixture(String(i), `Game ${i}`),
  );
  const services: ProviderId[] = [
    "steam",
    "igdb",
    "pcgw",
    "steamgriddb",
    "nexus",
    "reviews-steam",
    "reviews-gog",
  ];
  const called: ProviderId[] = [];
  const releases: ((value: ProviderResult) => void)[] = [];
  const queue = new EnrichmentQueue({
    getGames: () => games,
    concurrency: 12,
    perProviderConcurrency: 2,
    enabled: (p) => services.includes(p),
    order: (p) => services.indexOf(p),
    now: () => 1000,
    fetch: (_g, p) => {
      called.push(p);
      return new Promise((resolve) => releases.push(resolve));
    },
    apply: (id, p, record) => {
      const game = games.find((g) => g.id === id)!;
      game.enrichment = { ...game.enrichment, [p]: record };
    },
  });
  queue.reconcile(games);
  expect(called).toHaveLength(12);
  expect(called.slice(0, services.length)).toEqual(services);
  for (const provider of services)
    expect(called.filter((p) => p === provider).length).toBeLessThanOrEqual(2);
  releases[0](success(1000));
  await tick();
  expect(called).toHaveLength(13);
  const running = called.slice(1);
  expect(running).toHaveLength(12);
  for (const provider of services)
    expect(running.filter((p) => p === provider).length).toBeLessThanOrEqual(2);
  queue.clear();
});
