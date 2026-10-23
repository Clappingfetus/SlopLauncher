import { describe, it, expect } from "vitest";
import { ArtworkLoader } from "./artwork-loader";
import { artworkWindow, thumbnailWidth } from "./artwork-window";
import type { Game } from "./types";
const request = (url: string, priority = 0) => ({
  url,
  thumbnail: 256,
  priority,
});
const tick = async () => {
  for (let i = 0; i < 8; i++) await Promise.resolve();
};
describe("bounded decoded artwork", () => {
  it("retries failed offscreen lookahead on reconnection", async () => {
    let offline = true;
    const loader = new ArtworkLoader({
      resolve: async (url) => {
        if (offline) throw Error("offline");
        return url;
      },
      decode: async (src) => ({ src, bytes: 10 }),
    });
    loader.setWindow("grid", [request("offscreen", 2)]);
    await tick();
    expect(loader.peek("offscreen", 256)).toBeUndefined();
    offline = false;
    loader.reconnect();
    await tick();
    expect(loader.peek("offscreen", 256)).toBe("offscreen");
  });
  it("reuses decoded covers synchronously after cards unmount", async () => {
    let requests = 0,
      decodes = 0;
    const loader = new ArtworkLoader({
      resolve: async (url) => {
        requests++;
        return url;
      },
      decode: async (src) => {
        decodes++;
        return { src, bytes: 1024 };
      },
    });
    const first = loader.acquire(request("original"));
    expect(loader.peek("original", 256)).toBeUndefined();
    await first.promise;
    first.release();
    expect(loader.peek("original", 256)).toBe("original");
    const second = loader.acquire(request("original"));
    await second.promise;
    second.release();
    expect([requests, decodes]).toEqual([1, 1]);
  });
  it("reserves capacity for visible art and drops queued work after a jump", async () => {
    const started: string[] = [],
      finish = new Map<string, () => void>();
    const loader = new ArtworkLoader({
      resolve: (url) =>
        new Promise((r) => {
          started.push(url);
          finish.set(url, () => r(url));
        }),
      decode: async (src) => ({ src, bytes: 10 }),
    });
    loader.setWindow(
      "grid",
      Array.from({ length: 20 }, (_, i) => request(`old${i}`, 2)),
    );
    expect(started).toEqual(["old0", "old1"]);
    const visible = loader.acquire(request("visible"));
    expect(started).toContain("visible");
    loader.setWindow("grid", [request("new", 0)]);
    expect(started).toContain("new");
    expect(loader.stats().active).toBe(4);
    finish.forEach((f) => f());
    await tick();
    await visible.promise;
    visible.release();
    expect(started).not.toContain("old2");
    expect(loader.stats().queued).toBe(0);
  });
  it("evicts old decoded images to enforce entry and memory budgets", async () => {
    const disposed: string[] = [];
    const loader = new ArtworkLoader({
      maxBytes: 20,
      maxEntries: 3,
      resolve: async (url) => url,
      decode: async (src) => ({
        src,
        bytes: 10,
        dispose: () => {
          disposed.push(src);
        },
      }),
    });
    for (let i = 0; i < 30; i++) {
      const h = loader.acquire(request(`art${i}`));
      await h.promise;
      h.release();
    }
    expect(loader.stats().bytes).toBeLessThanOrEqual(20);
    expect(loader.stats().entries).toBeLessThanOrEqual(3);
    expect(disposed).toHaveLength(28);
    expect(loader.peek("art29", 256)).toBe("art29");
  });
  it("pins mounted covers instead of repeatedly evicting and decoding them", async () => {
    const loader = new ArtworkLoader({
      maxBytes: 10,
      resolve: async (url) => url,
      decode: async (src) => ({ src, bytes: 10 }),
    });
    const a = loader.acquire(request("a")),
      b = loader.acquire(request("b"));
    await Promise.all([a.promise, b.promise]);
    expect(loader.peek("a", 256)).toBe("a");
    expect(loader.peek("b", 256)).toBe("b");
    a.release();
    expect(loader.stats().bytes).toBe(10);
    b.release();
  });
  it("keys changed art by URL and size without poisoning late additions", async () => {
    const finishes = new Map<string, () => void>();
    const loader = new ArtworkLoader({
      resolve: (url, size) =>
        new Promise((r) =>
          finishes.set(`${url}:${size}`, () => r(`${url}:${size}`)),
        ),
      decode: async (src) => ({ src, bytes: 10 }),
    });
    const old = loader.acquire(request("old"));
    old.release();
    const next = loader.acquire(request("rematched"));
    const late = loader.acquire(request("late"));
    finishes.get("old:256")!();
    await tick();
    expect(loader.peek("rematched", 256)).toBeUndefined();
    finishes.get("rematched:256")!();
    finishes.get("late:256")!();
    await Promise.all([next.promise, late.promise]);
    expect(loader.peek("rematched", 256)).toBe("rematched:256");
    const bigger = loader.acquire({ ...request("rematched"), thumbnail: 512 });
    expect(loader.peek("rematched", 512)).toBe("rematched:256");
    finishes.get("rematched:512")!();
    await bigger.promise;
    expect(loader.peek("rematched", 512)).toBe("rematched:512");
    next.release();
    late.release();
    bigger.release();
  });
  it("backs off failures and allows reconnection retry", async () => {
    let now = 0,
      attempts = 0;
    const loader = new ArtworkLoader({
      now: () => now,
      resolve: async (url) => {
        if (++attempts === 1) throw Error("offline");
        return url;
      },
      decode: async (src) => ({ src, bytes: 10 }),
    });
    let a = loader.acquire(request("a"));
    await expect(a.promise).rejects.toThrow("offline");
    a.release();
    a = loader.acquire(request("a"));
    await expect(a.promise).rejects.toThrow("offline");
    a.release();
    expect(attempts).toBe(1);
    now = 61_000;
    a = loader.acquire(request("a"));
    await a.promise;
    a.release();
    expect(attempts).toBe(2);
    loader.retry("a", 256);
    a = loader.acquire(request("a"));
    await a.promise;
    a.release();
    expect(attempts).toBe(3);
  });
  it("a retired grid cannot clear a newer grid's work", () => {
    const loader = new ArtworkLoader({
      resolve: () => new Promise(() => {}),
      decode: async (src) => ({ src, bytes: 10 }),
    });
    loader.setWindow("old", [request("a", 2)]);
    loader.setWindow("new", [request("b", 2)]);
    loader.clearWindow("old");
    expect(loader.stats().wanted).toBe(1);
    loader.clearWindow("new");
    expect(loader.stats().wanted).toBe(0);
  });
});
describe("artwork lookahead", () => {
  const games = Array.from(
    { length: 10000 },
    (_, i) => ({ id: `${i}`, cover: `cover${i}` }) as Game,
  );
  it("stays local and within its decoded-byte allowance for 10,000 games", () => {
    for (const size of [256, 384, 512]) {
      const window = artworkWindow(games, 6, 500, 503, 1, size);
      expect(window.length).toBeLessThanOrEqual(128);
      expect(window.length * size * size * 1.5 * 4).toBeLessThanOrEqual(
        48 * 1024 * 1024,
      );
      expect(window.slice(0, 18).every((r) => r.priority === 0)).toBe(true);
      expect(
        window.every(
          (r) =>
            Number(r.url.slice(5)) >= 2982 && Number(r.url.slice(5)) < 3060,
        ),
      ).toBe(true);
    }
  });
  it("adapts to direction, late games, removals and replacement art", () => {
    expect(
      artworkWindow(games, 6, 50, 53, -1, 256).find(
        (r) => r.url === "cover258",
      ),
    ).toBeDefined();
    expect(
      artworkWindow(games, 6, 50, 53, 1, 256).find((r) => r.url === "cover354"),
    ).toBeDefined();
    const changed = [
      { id: "late", cover: "new" },
      { id: "rematched", cover: "replacement" },
      { id: "duplicate", cover: "new" },
    ] as Game[];
    expect(artworkWindow(changed, 6, 0, 1, 1, 256).map((r) => r.url)).toEqual([
      "new",
      "replacement",
    ]);
    expect(artworkWindow([], 6, 0, 0, 1, 256)).toEqual([]);
  });
  it("uses a few bounded resolution tiers for display scaling", () => {
    expect([
      thumbnailWidth(176, 1),
      thumbnailWidth(176, 2),
      thumbnailWidth(300, 3),
      thumbnailWidth(900, 1),
    ]).toEqual([256, 384, 512, 512]);
  });
});
