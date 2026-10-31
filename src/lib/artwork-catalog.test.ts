import { describe, it, expect, vi } from "vitest";
import { ArtworkCatalog, libraryCoverTargets } from "./artwork-catalog";
describe("persistent library thumbnail index", () => {
  it("indexes a massive library in bounded batches without decoding or downloading prepared covers", async () => {
    const lookup = vi.fn(async (urls: string[]) =>
      urls.map((url) => [url, `disk:${url}`] as [string, string]),
    );
    const prepare = vi.fn(async (url: string) => url);
    const catalog = new ArtworkCatalog({
      lookup,
      prepare,
      foregroundBusy: () => false,
      changed: () => {},
    });
    catalog.reconcile(Array.from({ length: 10000 }, (_, i) => String(i)));
    for (let i = 0; i < 80; i++) await catalog.step();
    expect(lookup.mock.calls.every(([urls]) => urls.length <= 128)).toBe(true);
    expect(catalog.peek("9999")).toBe("disk:9999");
    expect(catalog.stats().prepared).toBe(10000);
    expect(prepare).not.toHaveBeenCalled();
  });
  it("keeps one slot during foreground work and discards removed or rematched results", async () => {
    let busy = true,
      finish!: (path: string) => void;
    const prepare = vi.fn(() => new Promise<string>((r) => (finish = r)));
    const catalog = new ArtworkCatalog({
      lookup: async () => [],
      prepare,
      foregroundBusy: () => busy,
      changed: () => {},
    });
    catalog.reconcile(["old"]);
    await catalog.step();
    const pending = catalog.step();
    await catalog.step();
    expect(prepare).toHaveBeenCalledTimes(1);
    busy = false;
    catalog.reconcile(["replacement"]);
    finish("old-disk");
    await pending;
    expect(catalog.peek("old")).toBeUndefined();
    await catalog.step();
    const next = catalog.step();
    finish("new-disk");
    await next;
    expect(catalog.peek("replacement")).toBe("new-disk");
    catalog.reconcile([]);
    expect(catalog.stats().prepared).toBe(0);
  });
  it("backs off failed preparation without blocking later games and accepts late additions", async () => {
    const prepare = vi.fn(async (url: string) => {
      if (url === "bad") throw Error("offline");
      return `disk:${url}`;
    });
    const catalog = new ArtworkCatalog({
      lookup: async () => [],
      prepare,
      foregroundBusy: () => false,
      changed: () => {},
      now: () => 1,
    });
    catalog.reconcile(["bad", "good"]);
    await catalog.step();
    await catalog.step();
    await catalog.step();
    await catalog.step();
    expect(prepare).toHaveBeenCalledTimes(2);
    expect(catalog.peek("good")).toBe("disk:good");
    expect(catalog.stats()).toMatchObject({
      prepared: 1,
      retrying: 1,
      pending: 0,
      working: false,
      nextRetryAt: 3600001,
    });
    catalog.reconcile(["bad", "good", "late"]);
    await catalog.step();
    await catalog.step();
    expect(catalog.peek("late")).toBe("disk:late");
  });
});

it("bounds background downloads, credits foreground success, and retries failed covers", async () => {
  let time = 1;
  const releases = new Map<
    string,
    { resolve: (v: string) => void; reject: () => void }
  >();
  const catalog = new ArtworkCatalog({
    lookup: async () => [],
    prepare: (url) =>
      new Promise<string>((resolve, reject) =>
        releases.set(url, { resolve, reject: () => reject(Error("offline")) }),
      ),
    foregroundBusy: () => false,
    changed: () => {},
    now: () => time,
  });
  catalog.reconcile(["a", "b", "c"]);
  await catalog.step();
  const a = catalog.step(),
    b = catalog.step();
  await catalog.step();
  expect([...releases.keys()]).toEqual(["a", "b"]);
  catalog.remember("a", "foreground:a");
  releases.get("a")!.reject();
  releases.get("b")!.reject();
  await Promise.all([a, b]);
  expect(catalog.stats()).toMatchObject({
    prepared: 1,
    retrying: 1,
    pending: 1,
  });
  const c = catalog.step();
  releases.get("c")!.resolve("disk:c");
  await c;
  expect(catalog.stats()).toMatchObject({
    prepared: 2,
    retrying: 1,
    pending: 0,
  });
  time = 3600002;
  const retry = catalog.step();
  releases.get("b")!.resolve("disk:b");
  await retry;
  expect(catalog.stats()).toMatchObject({
    prepared: 3,
    retrying: 0,
    pending: 0,
    working: false,
  });
});

it("falls back to individual cache checks when batch discovery fails", async () => {
  const catalog = new ArtworkCatalog({
    lookup: async () => {
      throw Error("index unavailable");
    },
    prepare: async (url) => `disk:${url}`,
    foregroundBusy: () => false,
    changed: () => {},
  });
  catalog.reconcile(["cover"]);
  await catalog.step();
  await catalog.step();
  expect(catalog.stats()).toMatchObject({
    prepared: 1,
    pending: 0,
    retrying: 0,
  });
});

it("prepares off-screen fallback artwork without downloading unused alternatives", async () => {
  const prepare = vi.fn(async (url: string) => {
    if (url === "broken") throw Error("404");
    return `disk:${url}`;
  });
  const catalog = new ArtworkCatalog({
    lookup: async () => [],
    prepare,
    foregroundBusy: () => false,
    changed: () => {},
  });
  const targets = libraryCoverTargets([
    { cover: "broken", artworkFallbacks: { cover: ["alternate", "unused"] } },
    { cover: "healthy", artworkFallbacks: { cover: ["also-unused"] } },
  ]);
  catalog.reconcile([...targets.keys()], targets);
  await catalog.step();
  await catalog.step();
  await catalog.step();
  expect(prepare.mock.calls.map(([url]) => url)).toEqual([
    "broken",
    "alternate",
    "healthy",
  ]);
  expect(catalog.peek("broken")).toBe("disk:alternate");
  expect(catalog.source("broken")).toBe("alternate");
  expect(catalog.stats()).toMatchObject({ total: 2, prepared: 2, retrying: 0 });
});

it("rejects obsolete fallback results and schedules changed candidates", async () => {
  let finish!: (path: string) => void;
  const catalog = new ArtworkCatalog({
    lookup: async () => [],
    prepare: async (url) => {
      if (url === "broken") throw Error("404");
      if (url === "old")
        return new Promise<string>((resolve) => (finish = resolve));
      return `disk:${url}`;
    },
    foregroundBusy: () => false,
    changed: () => {},
  });
  catalog.reconcile(["broken"], new Map([["broken", ["old"]]]));
  await catalog.step();
  const pending = catalog.step();
  await Promise.resolve();
  await Promise.resolve();
  catalog.reconcile(["broken"], new Map([["broken", ["new"]]]));
  finish("disk:old");
  await pending;
  expect(catalog.peek("broken")).toBeUndefined();
  await catalog.step();
  expect(catalog.peek("broken")).toBe("disk:new");
  catalog.reconcile(["broken"], new Map([["broken", ["newer"]]]));
  expect(catalog.peek("broken")).toBeUndefined();
  await catalog.step();
  await catalog.step();
  expect(catalog.peek("broken")).toBe("disk:newer");
});

it("includes fallback-only and late games in shared cover targets", () => {
  const targets = libraryCoverTargets([
    { cover: "shared", artworkFallbacks: { cover: ["a"] } },
    { cover: "shared", artworkFallbacks: { cover: ["b", "a"] } },
    { artworkFallbacks: { cover: ["late", "alternative"] } },
  ]);
  expect([...targets]).toEqual([
    ["shared", ["a", "b"]],
    ["late", ["alternative"]],
  ]);
});

it("finds persisted alternate covers on restart without making a network request", async () => {
  const prepare = vi.fn(async () => {
    throw Error("must not download");
  });
  const catalog = new ArtworkCatalog({
    lookup: async (urls) =>
      urls.includes("cached-alternate")
        ? [["cached-alternate", "disk:alternate"]]
        : [],
    prepare,
    foregroundBusy: () => false,
    changed: () => {},
  });
  catalog.reconcile(["broken"], new Map([["broken", ["cached-alternate"]]]));
  await catalog.step();
  await catalog.step();
  expect(catalog.peek("broken")).toBe("disk:alternate");
  expect(catalog.source("broken")).toBe("cached-alternate");
  expect(prepare).not.toHaveBeenCalled();
});

it("primes the initial screen and hydrates cached paths without downloads or timer-per-batch delays", async () => {
  const lookup = vi.fn(async (urls: string[]) =>
    urls.map((url) => [url, `disk:${url}`] as [string, string]),
  );
  const prepare = vi.fn(async (url: string) => url);
  const catalog = new ArtworkCatalog({
    lookup,
    prepare,
    foregroundBusy: () => false,
    changed: () => {},
  });
  catalog.reconcile(Array.from({ length: 300 }, (_, i) => String(i)));
  await catalog.prime(["299", "298"]);
  expect(catalog.peek("299")).toBe("disk:299");
  expect(catalog.peek("0")).toBeUndefined();
  await catalog.hydrate();
  expect(catalog.stats().prepared).toBe(300);
  expect(prepare).not.toHaveBeenCalled();
  expect(lookup.mock.calls.every(([urls]) => urls.length <= 128)).toBe(true);
});
it("rejects a stale startup path after the game artwork is removed", async () => {
  let finish!: (rows: [string, string][]) => void;
  const catalog = new ArtworkCatalog({
    lookup: () => new Promise((r) => (finish = r)),
    prepare: async (u) => u,
    foregroundBusy: () => false,
    changed: () => {},
  });
  catalog.reconcile(["old"]);
  const prime = catalog.prime(["old"]);
  catalog.reconcile(["new"]);
  finish([["old", "old-path"]]);
  await prime;
  expect(catalog.peek("old")).toBeUndefined();
  expect(catalog.stats().prepared).toBe(0);
});
it("restores prepared paths, fallbacks and failure deadlines without another lookup or download", async () => {
  let now = 1000;
  const stored = new Map<
    string,
    import("./artwork-catalog").ArtworkCatalogRecord
  >();
  const lookup = vi.fn(async (urls: string[]) =>
    urls
      .filter((u) => u === "fallback")
      .map((u) => [u, "disk:fallback"] as [string, string]),
  );
  const prepare = vi.fn(async () => {
    throw Error("Offline");
  });
  const persist = vi.fn((r: import("./artwork-catalog").ArtworkCatalogRecord) =>
    stored.set(r.url, structuredClone(r)),
  );
  const options = {
    lookup,
    prepare,
    foregroundBusy: () => false,
    changed: () => {},
    now: () => now,
    persist,
  };
  const first = new ArtworkCatalog(options);
  first.reconcile(["primary", "failed"], new Map([["primary", ["fallback"]]]));
  await first.hydrate();
  await first.step();
  expect(first.stats().retrying).toBe(1);
  const saved = [...stored.values()];
  lookup.mockClear();
  prepare.mockClear();
  persist.mockClear();
  const reopened = new ArtworkCatalog(options);
  reopened.reconcile(
    ["primary", "failed"],
    new Map([["primary", ["fallback"]]]),
  );
  reopened.restore(JSON.parse(JSON.stringify(saved)));
  await reopened.hydrate();
  await reopened.step();
  expect(reopened.peek("primary")).toBe("disk:fallback");
  expect(reopened.source("primary")).toBe("fallback");
  expect(reopened.stats().retrying).toBe(1);
  expect(lookup).not.toHaveBeenCalled();
  expect(prepare).not.toHaveBeenCalled();
  expect(persist).not.toHaveBeenCalled();
  now += 3_600_001;
  await reopened.step();
  expect(prepare).toHaveBeenCalledTimes(1);
  expect(reopened.peek("primary")).toBe("disk:fallback");
});
it("only discovers late covers and repairs invalidated entries after a restored cache", async () => {
  const lookup = vi.fn(async () => []);
  const prepare = vi.fn(async (u: string) => `disk:${u}`);
  const changes: any[] = [];
  const catalog = new ArtworkCatalog({
    lookup,
    prepare,
    foregroundBusy: () => false,
    changed: () => {},
    persist: (r) => changes.push(r),
  });
  catalog.reconcile(["old", "keep"]);
  catalog.restore([
    { url: "old", path: "old-disk" },
    { url: "keep", path: "keep-disk" },
  ]);
  catalog.reconcile(["old", "keep", "late"]);
  await catalog.hydrate();
  expect(lookup.mock.calls).toEqual([[["late"]]]);
  await catalog.step();
  expect(prepare.mock.calls).toEqual([["late"]]);
  catalog.invalidate("old");
  await catalog.step();
  expect(prepare.mock.calls).toEqual([["late"], ["old"]]);
  expect(catalog.peek("keep")).toBe("keep-disk");
  catalog.reconcile(["keep", "late"]);
  expect([...changes].reverse().find((r) => r.url === "old")).toMatchObject({
    url: "old",
    path: undefined,
  });
});
it("ignores restored paths when artwork identity or its permitted fallback changed", async () => {
  const lookup = vi.fn(async () => []);
  const catalog = new ArtworkCatalog({
    lookup,
    prepare: async (u) => u,
    foregroundBusy: () => false,
    changed: () => {},
  });
  catalog.reconcile(["current"], new Map([["current", ["new-fallback"]]]));
  catalog.restore([
    { url: "removed", path: "removed-path" },
    { url: "current", path: "stale-path", source: "old-fallback" },
  ]);
  expect(catalog.stats().prepared).toBe(0);
  await catalog.hydrate();
  expect(lookup).toHaveBeenCalledWith(["current", "new-fallback"]);
});
it("keeps failed covers settled while retrying and restores overdue retries without rebuilding cached covers", async () => {
  let now = 1000,
    finish!: (path: string) => void;
  const options = {
    lookup: vi.fn(async () => []),
    prepare: vi.fn(() => new Promise<string>((r) => (finish = r))),
    foregroundBusy: () => false,
    changed: () => {},
    now: () => now,
  };
  const c = new ArtworkCatalog(options);
  c.reconcile(["cached", "failed"]);
  c.restore([
    { url: "cached", path: "disk:cached" },
    { url: "failed", retryAt: 500 },
  ]);
  expect(c.stats()).toMatchObject({ prepared: 1, retrying: 1, pending: 0 });
  const work = c.step();
  expect(c.stats()).toMatchObject({
    prepared: 1,
    retrying: 1,
    pending: 0,
    working: true,
  });
  finish("disk:failed");
  await work;
  expect(c.stats()).toMatchObject({ prepared: 2, retrying: 0, pending: 0 });
  expect(options.lookup).not.toHaveBeenCalled();
});
