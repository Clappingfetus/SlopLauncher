// Keep only URL/path strings for the whole library; decoded pixels remain bounded
// by ArtworkLoader. Every async result is checked against the current URL set.
export class ArtworkCatalog {
  private urls = new Set<string>();
  private paths = new Map<string, string>();
  private unchecked = new Set<string>();
  private retries = new Map<string, number>();
  private lookingUp = false;
  private active = new Set<string>();
  constructor(
    private options: {
      lookup: (urls: string[]) => Promise<[string, string][]>;
      prepare: (url: string) => Promise<string>;
      foregroundBusy: () => boolean;
      changed: () => void;
      now?: () => number;
    },
  ) {}
  reconcile(urls: string[]) {
    const next = new Set(urls.filter(Boolean));
    for (const url of next) if (!this.urls.has(url)) this.unchecked.add(url);
    for (const url of this.urls)
      if (!next.has(url)) {
        this.paths.delete(url);
        this.unchecked.delete(url);
        this.retries.delete(url);
      }
    this.urls = next;
    this.options.changed();
  }
  peek(url: string) {
    return this.paths.get(url);
  }
  remember(url: string, path: string) {
    if (!this.urls.has(url)) return;
    this.paths.set(url, path);
    this.unchecked.delete(url);
    this.retries.delete(url);
    this.options.changed();
  }
  invalidate(url: string) {
    this.paths.delete(url);
    this.retries.delete(url);
    this.options.changed();
  }
  stats() {
    return {
      total: this.urls.size,
      prepared: this.paths.size,
      working: this.lookingUp || this.active.size > 0,
      retrying: this.retries.size,
      pending: this.urls.size - this.paths.size - this.retries.size,
      nextRetryAt: this.retries.size
        ? Math.min(...this.retries.values())
        : undefined,
    };
  }
  async step() {
    if (this.lookingUp || !this.urls.size) return;
    const batch = [...this.unchecked].slice(0, 128);
    // Local path discovery does not compete with downloads or image decoding.
    if (batch.length) {
      this.lookingUp = true;
      try {
        const found = await this.options.lookup(batch);
        for (const url of batch) this.unchecked.delete(url);
        for (const [url, path] of found)
          if (this.urls.has(url)) {
            this.paths.set(url, path);
            this.retries.delete(url);
          }
      } catch {
        // The single-item preparation path also checks disk before downloading.
        // A broken batch lookup must not block the entire library indefinitely.
        for (const url of batch) this.unchecked.delete(url);
      } finally {
        this.lookingUp = false;
        this.options.changed();
      }
      return;
    }
    // Keep one background slot during browsing so scrolling cannot starve the
    // shared cache. When idle, two bounded jobs can overlap network waits.
    const limit = this.options.foregroundBusy() ? 1 : 2;
    if (this.active.size >= limit) return;
    const now = (this.options.now ?? Date.now)();
    const url = [...this.urls].find(
      (url) =>
        !this.paths.has(url) &&
        !this.active.has(url) &&
        (this.retries.get(url) ?? 0) <= now,
    );
    if (!url) return;
    this.active.add(url);
    this.retries.delete(url);
    this.options.changed();
    try {
      this.remember(url, await this.options.prepare(url));
    } catch {
      // Foreground preparation may have succeeded while this request failed.
      if (this.urls.has(url) && !this.paths.has(url))
        this.retries.set(url, (this.options.now ?? Date.now)() + 3_600_000);
    } finally {
      this.active.delete(url);
      this.options.changed();
    }
  }
}
