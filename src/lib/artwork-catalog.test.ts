import { describe, it, expect, vi } from "vitest";
import { ArtworkCatalog } from "./artwork-catalog";
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
