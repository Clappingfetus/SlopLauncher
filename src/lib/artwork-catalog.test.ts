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
  it("yields preparation to foreground work, and discards removed or rematched results", async () => {
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
    await catalog.step();
    expect(prepare).not.toHaveBeenCalled();
    busy = false;
    const pending = catalog.step();
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
    catalog.reconcile(["bad", "good", "late"]);
    await catalog.step();
    await catalog.step();
    expect(catalog.peek("late")).toBe("disk:late");
  });
});
