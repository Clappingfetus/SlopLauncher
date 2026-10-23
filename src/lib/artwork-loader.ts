export interface ArtworkRequest {
  url: string;
  thumbnail: number;
  priority: number;
}
export interface DecodedArtwork {
  src: string;
  bytes: number;
  retain?: unknown;
  dispose?: () => void;
}
type Entry = ArtworkRequest & {
  key: string;
  users: number;
  touched: number;
  state: "queued" | "loading" | "ready" | "failed";
  value?: DecodedArtwork;
  retryAt: number;
  promise: Promise<string>;
  resolve: (src: string) => void;
  reject: (error: Error) => void;
};
interface Options {
  resolve: (url: string, thumbnail: number) => Promise<string>;
  decode: (src: string) => Promise<DecodedArtwork>;
  maxBytes?: number;
  maxEntries?: number;
  concurrency?: number;
  now?: () => number;
}
const keyOf = (url: string, thumbnail: number) =>
  JSON.stringify([thumbnail, url]);
export class ArtworkLoader {
  private entries = new Map<string, Entry>();
  private wanted = new Map<string, ArtworkRequest>();
  private owner: unknown;
  private active = 0;
  private speculative = 0;
  private clock = 0;
  constructor(private options: Options) {}
  peek(url: string, thumbnail = 0): string | undefined {
    const exact = this.entries.get(keyOf(url, thumbnail));
    if (exact?.state === "ready") {
      exact.touched = ++this.clock;
      return exact.value!.src;
    }
    // Reuse a warm version during a grid resize while its new size is prepared.
    for (const e of this.entries.values())
      if (e.url === url && e.state === "ready") {
        e.touched = ++this.clock;
        return e.value!.src;
      }
    return undefined;
  }
  acquire(request: ArtworkRequest) {
    const entry = this.ensure(request);
    entry.users++;
    entry.priority = Math.min(entry.priority, request.priority);
    this.pump();
    let released = false;
    return {
      promise: entry.promise,
      release: () => {
        if (released) return;
        released = true;
        entry.users--;
        this.prune();
        this.trim();
      },
    };
  }
  setWindow(owner: unknown, requests: ArtworkRequest[]) {
    this.owner = owner;
    this.wanted = new Map(
      requests.slice(0, 128).map((r) => [keyOf(r.url, r.thumbnail), r]),
    );
    this.prune();
    for (const r of this.wanted.values()) {
      const e = this.ensure(r);
      e.priority = e.users ? Math.min(e.priority, r.priority) : r.priority;
    }
    this.trim();
    this.pump();
  }
  clearWindow(owner: unknown) {
    if (this.owner === owner) {
      this.wanted.clear();
      this.owner = undefined;
      this.prune();
      this.trim();
    }
  }
  retry(url: string, thumbnail = 0) {
    const key = keyOf(url, thumbnail),
      e = this.entries.get(key);
    if (e && e.state !== "loading") this.remove(e);
  }
  reconnect() {
    // Offscreen prefetches also need another chance when connectivity returns.
    for (const e of this.entries.values())
      if (e.state === "failed") this.remove(e);
    for (const r of this.wanted.values()) this.ensure(r);
    this.pump();
  }
  stats() {
    const values = [...this.entries.values()];
    return {
      active: this.active,
      queued: values.filter((e) => e.state === "queued").length,
      entries: values.length,
      ready: values.filter((e) => e.state === "ready").length,
      bytes: values.reduce((n, e) => n + (e.value?.bytes ?? 0), 0),
      wanted: this.wanted.size,
    };
  }
  private ensure(r: ArtworkRequest): Entry {
    const key = keyOf(r.url, r.thumbnail),
      old = this.entries.get(key);
    if (
      old &&
      !(
        old.state === "failed" &&
        old.retryAt <= (this.options.now ?? Date.now)()
      )
    )
      return old;
    if (old) this.remove(old);
    let resolve!: (src: string) => void, reject!: (error: Error) => void;
    const promise = new Promise<string>((yes, no) => {
      resolve = yes;
      reject = no;
    });
    // Speculative requests have no component awaiting their promise.
    void promise.catch(() => {});
    const entry: Entry = {
      ...r,
      key,
      users: 0,
      touched: ++this.clock,
      state: "queued",
      retryAt: 0,
      promise,
      resolve,
      reject,
    };
    this.entries.set(key, entry);
    return entry;
  }
  private prune() {
    for (const e of this.entries.values())
      if (e.state === "queued" && !e.users && !this.wanted.has(e.key)) {
        this.entries.delete(e.key);
        e.reject(new Error("Artwork no longer in view"));
      }
  }
  private remove(e: Entry) {
    this.entries.delete(e.key);
    e.value?.dispose?.();
  }
  private trim() {
    const max = this.options.maxEntries ?? 128,
      budget = this.options.maxBytes ?? 64 * 1024 * 1024;
    let stats = this.stats();
    const candidates = [...this.entries.values()]
      .filter((e) => !e.users && e.state !== "loading" && e.state !== "queued")
      .sort(
        (a, b) =>
          (this.wanted.get(b.key)?.priority ?? 100) -
            (this.wanted.get(a.key)?.priority ?? 100) || a.touched - b.touched,
      );
    for (const e of candidates) {
      if (stats.entries <= max && stats.bytes <= budget) break;
      this.remove(e);
      stats.entries--;
      stats.bytes -= e.value?.bytes ?? 0;
    }
  }
  private pump() {
    while (this.active < (this.options.concurrency ?? 4)) {
      const next = [...this.entries.values()]
        .filter(
          (e) =>
            e.state === "queued" &&
            (e.users || this.wanted.has(e.key)) &&
            (e.priority < 2 || this.speculative < 2),
        )
        .sort((a, b) => a.priority - b.priority || a.touched - b.touched)[0];
      if (!next) return;
      next.state = "loading";
      this.active++;
      const speculative = next.priority >= 2;
      if (speculative) this.speculative++;
      void this.run(next, speculative);
    }
  }
  private async run(e: Entry, speculative: boolean) {
    try {
      const src = await this.options.resolve(e.url, e.thumbnail);
      const value = await this.options.decode(src);
      e.value = value;
      e.state = "ready";
      e.touched = ++this.clock;
      e.resolve(value.src);
    } catch (error) {
      e.state = "failed";
      e.retryAt = (this.options.now ?? Date.now)() + 60_000;
      e.reject(
        error instanceof Error ? error : new Error("Artwork unavailable"),
      );
    } finally {
      this.active--;
      if (speculative) this.speculative--;
      this.trim();
      this.pump();
    }
  }
}

export async function decodeArtwork(src: string): Promise<DecodedArtwork> {
  const img = new Image();
  img.decoding = "async";
  img.src = src;
  try {
    await img.decode();
  } catch {
    img.src = "";
    throw new Error("Artwork could not be decoded");
  }
  return {
    src,
    bytes: img.naturalWidth * img.naturalHeight * 4,
    retain: img,
    dispose: () => {
      img.src = "";
    },
  };
}
