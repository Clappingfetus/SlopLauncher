// Match the cover candidates used by cards, including duplicate-store fallbacks.
export function libraryCoverTargets(
  games: { cover?: string; artworkFallbacks?: { cover?: string[] } }[],
) {
  const targets = new Map<string, string[]>();
  for (const game of games) {
    const candidates = [
      ...new Set(
        [game.cover, ...(game.artworkFallbacks?.cover ?? [])].filter(
          (url): url is string => !!url,
        ),
      ),
    ];
    const [primary, ...fallbacks] = candidates;
    if (primary)
      targets.set(primary, [
        ...new Set([...(targets.get(primary) ?? []), ...fallbacks]),
      ]);
  }
  return targets;
}

export interface ArtworkCatalogRecord {
  url: string;
  path?: string;
  source?: string;
  retryAt?: number;
}

// Keep only URL/path strings for the whole library; decoded pixels remain bounded
// by ArtworkLoader. Every async result is checked against the current URL set.
export class ArtworkCatalog {
  private urls = new Set<string>();
  private paths = new Map<string, string>();
  private fallbacks = new Map<string, string[]>();
  private sources = new Map<string, string>();
  private unchecked = new Set<string>();
  private retries = new Map<string, number>();
  private lookingUp = false;
  private active = new Set<string>();
  private restoring = false;
  constructor(
    private options: {
      lookup: (urls: string[]) => Promise<[string, string][]>;
      prepare: (url: string) => Promise<string>;
      foregroundBusy: () => boolean;
      changed: () => void;
      now?: () => number;
      persist?: (record: ArtworkCatalogRecord) => void;
    },
  ) {}
  private record(url: string) {
    if (!this.restoring)
      this.options.persist?.({
        url,
        path: this.paths.get(url),
        source: this.sources.get(url),
        retryAt: this.retries.get(url),
      });
  }
  restore(records: ArtworkCatalogRecord[]) {
    this.restoring = true;
    try {
      for (const record of records) {
        if (!this.urls.has(record.url)) continue;
        const source = record.source ?? record.url;
        if (
          source !== record.url &&
          !this.fallbacks.get(record.url)?.includes(source)
        )
          continue;
        if (record.path) this.remember(record.url, record.path, source);
        else if (
          record.retryAt &&
          record.retryAt > (this.options.now ?? Date.now)()
        ) {
          this.retries.set(record.url, record.retryAt);
          this.unchecked.delete(record.url);
        }
      }
    } finally {
      this.restoring = false;
      this.options.changed();
    }
  }
  reconcile(urls: string[], fallbacks = new Map<string, string[]>()) {
    const next = new Set(urls.filter(Boolean));
    for (const url of next) {
      if (!this.urls.has(url)) this.unchecked.add(url);
      if (
        JSON.stringify(this.fallbacks.get(url)) !==
        JSON.stringify(fallbacks.get(url))
      ) {
        this.retries.delete(url);
        const source = this.sources.get(url);
        if (source && source !== url && !fallbacks.get(url)?.includes(source)) {
          this.paths.delete(url);
          this.sources.delete(url);
          this.unchecked.add(url);
        }
        if (this.urls.has(url)) this.record(url);
      }
    }
    this.fallbacks = new Map(fallbacks);
    for (const url of this.urls)
      if (!next.has(url)) {
        this.paths.delete(url);
        this.sources.delete(url);
        this.unchecked.delete(url);
        this.retries.delete(url);
        this.record(url);
      }
    this.urls = next;
    this.options.changed();
  }
  peek(url: string) {
    return this.paths.get(url);
  }
  source(url: string) {
    return this.sources.get(url) ?? url;
  }
  remember(url: string, path: string, source = url) {
    if (
      !this.urls.has(url) ||
      (source !== url && !this.fallbacks.get(url)?.includes(source))
    )
      return;
    const changed =
      this.paths.get(url) !== path ||
      this.sources.get(url) !== source ||
      this.retries.has(url);
    this.paths.set(url, path);
    this.sources.set(url, source);
    this.unchecked.delete(url);
    this.retries.delete(url);
    if (changed) this.record(url);
    if (!this.restoring) this.options.changed();
  }
  invalidate(url: string) {
    this.paths.delete(url);
    this.sources.delete(url);
    this.retries.delete(url);
    this.record(url);
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
  // Resolve a foreground batch before cards mount. This only looks up existing
  // files: startup never waits for provider requests or missing-cover downloads.
  async prime(urls: string[]) {
    if (this.lookingUp) return;
    const batch = [...new Set(urls)]
      .filter((url) => this.urls.has(url) && this.unchecked.has(url))
      .slice(0, 128);
    if (!batch.length) return;
    this.lookingUp = true;
    try {
      // Resolve cached alternatives on restart too, without retrying a dead
      // preferred URL before discovering an already prepared fallback.
      const candidates = [
        ...new Set(
          batch.flatMap((url) => [url, ...(this.fallbacks.get(url) ?? [])]),
        ),
      ];
      const found = new Map<string, string>();
      for (let i = 0; i < candidates.length; i += 128)
        for (const [url, path] of await this.options.lookup(
          candidates.slice(i, i + 128),
        ))
          found.set(url, path);
      for (const url of batch) {
        this.unchecked.delete(url);
        const source = [url, ...(this.fallbacks.get(url) ?? [])].find(
          (candidate) => found.has(candidate),
        );
        if (source) this.remember(url, found.get(source)!, source);
      }
    } catch {
      // The single-item preparation path also checks disk before downloading.
      // A broken batch lookup must not block the entire library indefinitely.
      for (const url of batch) this.unchecked.delete(url);
    } finally {
      this.lookingUp = false;
      this.options.changed();
    }
  }
  async hydrate() {
    // Yield between local batches instead of imposing 250ms of idle time on
    // every 128 covers. Decoded images still use the bounded foreground loader.
    while (this.unchecked.size) {
      if (this.lookingUp) return;
      await this.prime([...this.unchecked].slice(0, 128));
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
  }
  async step() {
    if (this.lookingUp || !this.urls.size) return;
    const batch = [...this.unchecked].slice(0, 128);
    if (batch.length) {
      await this.prime(batch);
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
    const candidates = [url, ...(this.fallbacks.get(url) ?? [])];
    try {
      // Try alternatives off-screen too, but never download unused alternatives
      // when the preferred artwork succeeds. Stop if the current identity changes.
      for (const candidate of candidates) {
        if (!this.urls.has(url)) return;
        if (candidate !== url && !this.fallbacks.get(url)?.includes(candidate))
          continue;
        try {
          const path = await this.options.prepare(candidate);
          this.remember(url, path, candidate);
          return;
        } catch {
          if (this.paths.has(url)) return;
        }
      }
      throw Error("No cover candidate available");
    } catch {
      // Foreground preparation may have succeeded while this request failed.
      if (
        this.urls.has(url) &&
        !this.paths.has(url) &&
        JSON.stringify(candidates) ===
          JSON.stringify([url, ...(this.fallbacks.get(url) ?? [])])
      ) {
        this.retries.set(url, (this.options.now ?? Date.now)() + 3_600_000);
        this.record(url);
      }
    } finally {
      this.active.delete(url);
      this.options.changed();
    }
  }
}
