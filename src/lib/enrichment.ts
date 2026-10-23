import {
  providerIds,
  type Game,
  type ProviderId,
  type ProviderResult,
  type EnrichmentRecord,
} from "./types";
import { mergeTags } from "./tags";
import { preferredCopy } from "./library";
import { storeLink } from "./stores";
import { isEmulated, selectedArtwork } from "./artwork";
import { artworkCandidates } from "./store-artwork";
import {
  currentResult,
  identity,
  steamId,
  copyIdentities,
} from "./enrichment-identity";
export {
  currentResult,
  identity,
  steamId,
  copyIdentities,
} from "./enrichment-identity";
export function resourceLinks(game: Game, revision = 0) {
  const retro = currentResult(game, "retroachievements", revision)?.data
    ?.achievements;
  const wiki = currentResult(game, "pcgw");
  const hltb = currentResult(game, "hltb");
  const nexus = currentResult(game, "nexus")?.data;
  const nexusDomain = nexus?.nexusDomain;
  const copy = preferredCopy(game);
  const steamCopyId = Number(
    copy?.source === "steam" && /^\d+$/.test(copy.sourceId)
      ? copy.sourceId
      : undefined,
  );
  const shop = storeLink(game, currentResult(game, "stores")?.data?.storePages);
  const steam =
    Number.isSafeInteger(steamCopyId) &&
    steamCopyId > 0 &&
    steamCopyId <= 0xffffffff
      ? steamCopyId
      : undefined;
  const appid = steamId(game);
  const gog = game.copies.find(
    (c) => c.source === "gog" && /^\d+$/.test(c.sourceId),
  )?.sourceId;
  const wikiFallback = appid
    ? `https://www.pcgamingwiki.com/api/appid.php?appid=${appid}`
    : gog
      ? `https://www.pcgamingwiki.com/api/gog.php?page=${gog}`
      : `https://www.pcgamingwiki.com/w/index.php?search=${encodeURIComponent(game.title)}&title=Special%3ASearch`;
  const hltbId = game.providerIds?.hltb ?? wiki?.data?.hltbId;
  return {
    retroAchievements:
      retro?.source === "retroachievements" &&
      Number.isSafeInteger(retro.appId) &&
      retro.appId > 0
        ? `https://retroachievements.org/game/${retro.appId}`
        : undefined,
    pcgw: wiki?.data?.url ?? wikiFallback,
    hltb:
      hltb?.data?.url ??
      (hltbId
        ? `https://howlongtobeat.com/game/${hltbId}`
        : `https://howlongtobeat.com/?q=${encodeURIComponent(game.title)}`),
    wikiResolved: wiki?.status === "ready",
    hltbResolved: hltb?.status === "ready",
    nexus:
      nexusDomain &&
      /^[a-z0-9][a-z0-9-]{0,99}$/.test(nexusDomain) &&
      (nexus?.modCount ?? 0) > 0
        ? `https://www.nexusmods.com/${nexusDomain}/mods/`
        : undefined,
    discussions: steam
      ? `https://steamcommunity.com/app/${steam}/discussions/`
      : undefined,
    guides: steam
      ? `https://steamcommunity.com/app/${steam}/guides/`
      : undefined,
    store: shop?.url,
    storeName: shop?.name,
    storeLabel: shop?.label,
  };
}
export function gameTags(game: Game, revision = 0): string[] {
  const imported = game.metadataLocked
    ? []
    : providerIds.flatMap((provider) => {
        const data = currentResult(game, provider, revision)?.data;
        return [...(data?.tags ?? []), ...(data?.genres ?? [])];
      });
  return mergeTags(game.genres, game.tags, imported);
}
export function presentGame(game: Game, revision = 0): Game {
  const tags = gameTags(game, revision);
  const igdb = !game.metadataLocked
    ? currentResult(game, "igdb", revision)?.data
    : undefined;
  const ss =
    !game.metadataLocked && isEmulated(game)
      ? currentResult(game, "screenscraper", revision)?.data
      : undefined;
  const native = !game.metadataLocked
    ? game.copies.some((c) => c.source === "gog")
      ? currentResult(game, "stores", revision)?.data
      : !isEmulated(game)
        ? currentResult(game, "steam", revision)?.data
        : undefined
    : undefined;
  const releaseYear = game.year || ss?.year || native?.year || igdb?.year;
  const releasedAt =
    game.releasedAt ??
    [ss?.releasedAt, native?.releasedAt, igdb?.releasedAt].find(
      (time) =>
        typeof time === "number" &&
        (!releaseYear || new Date(time).getUTCFullYear() === releaseYear),
    );
  return {
    ...game,
    tags,
    cover:
      selectedArtwork(game, "cover")?.url ??
      (isEmulated(game)
        ? (ss?.cover ?? game.cover)
        : game.metadataLocked
          ? game.cover
          : artworkCandidates(game, "cover", revision)[0]?.url ||
            native?.cover),
    hero:
      selectedArtwork(game, "hero")?.url ??
      (isEmulated(game)
        ? (ss?.hero ?? game.hero)
        : game.metadataLocked
          ? game.hero
          : artworkCandidates(game, "hero", revision)[0]?.url || native?.hero),
    description:
      game.description || ss?.summary || native?.summary || igdb?.summary,
    developer:
      game.developer || ss?.developer || native?.developer || igdb?.developer,
    publisher:
      game.publisher || ss?.publisher || native?.publisher || igdb?.publisher,
    year: game.year || ss?.year || native?.year || igdb?.year,
    releasedAt,
    genres: game.genres.length
      ? game.genres
      : ss?.genres?.length
        ? ss.genres
        : native?.genres?.length
          ? native.genres
          : igdb?.genres?.length
            ? igdb.genres
            : game.genres,
    screenshots: game.screenshots.length
      ? game.screenshots
      : native?.screenshots?.length
        ? native.screenshots
        : igdb?.screenshots?.length
          ? igdb.screenshots
          : game.screenshots,
  };
}
// App state replaces changed game objects; unchanged rows retain their presentation.
export function createGamePresenter() {
  const cache = new WeakMap<Game, { revision: number; shown: Game }>();
  return (game: Game, revision = 0) => {
    const hit = cache.get(game);
    if (hit?.revision === revision) return hit.shown;
    const shown = presentGame(game, revision);
    cache.set(game, { revision, shown });
    return shown;
  };
}
export function hours(value: number | undefined): string {
  if (value === undefined || !Number.isFinite(value) || value <= 0) return "—";
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(value)} h`;
}

type Job = {
  game: Game;
  provider: ProviderId;
  fingerprint: string;
  key: string;
  generation: number;
};
export interface EnrichmentProgress {
  completed: number;
  queued: number;
  running: number;
  total: number;
  active: { title: string; provider: ProviderId }[];
}
interface QueueOptions {
  revisionFor?: (provider: ProviderId) => number;
  getGames: () => Game[];
  getGame?: (id: string) => Game | undefined;
  policy?: () => string;
  perProviderConcurrency?: number;
  fetch: (game: Game, provider: ProviderId) => Promise<ProviderResult>;
  apply: (id: string, provider: ProviderId, value: EnrichmentRecord) => void;
  now?: () => number;
  concurrency?: number;
  enabled?: (provider: ProviderId, game: Game) => boolean;
  order?: (provider: ProviderId) => number;
  onProgress?: (progress: EnrichmentProgress) => void;
}
export class EnrichmentQueue {
  private pending = new Map<string, Job>();
  private active = new Set<string>();
  private generation = 0;
  private revision = 0;
  private priority: string | null = null;
  private enabled = true;
  private running = 0;
  private jobs = new Map<string, Job>();
  private completed = 0;
  private idle = true;
  private lastProgress = "";
  private known = new Map<
    string,
    {
      game: Game;
      policy?: string;
      signature: string;
      enrichment: Game["enrichment"];
      revision: number;
      enabled: string;
      expires: number;
    }
  >();
  private pendingByGame = new Map<string, Set<string>>();
  private buckets = new Map<ProviderId, Map<string, Job>>();
  private scheduled: ReturnType<typeof setTimeout> | undefined;
  private request:
    { games: Game[]; revision: number; priority: string | null } | undefined;
  private scanEpoch = 0;
  constructor(private options: QueueOptions) {}
  private providerRevision(provider: ProviderId) {
    return this.options.revisionFor?.(provider) ?? this.revision;
  }
  private current(id: string) {
    return this.options.getGame
      ? this.options.getGame(id)
      : this.options.getGames().find((g) => g.id === id);
  }
  // Coalesce reactive changes and yield between short slices, including at startup.
  scheduleReconcile(
    games: Game[],
    revision = 0,
    priority: string | null = null,
  ) {
    this.request = { games, revision, priority };
    if (this.scheduled !== undefined) return;
    const epoch = this.scanEpoch;
    const begin = () => {
      if (epoch !== this.scanEpoch) return;
      const request = this.request;
      this.request = undefined;
      if (!request) {
        this.scheduled = undefined;
        return;
      }
      this.enabled = true;
      this.revision = request.revision;
      this.priority = request.priority;
      const ids = new Set(request.games.map((g) => g.id));
      for (const id of this.known.keys())
        if (!ids.has(id)) {
          this.dropPending(id);
          this.known.delete(id);
        }
      if (request.priority) {
        const game = this.current(request.priority);
        if (game) this.refresh(game, (this.options.now ?? Date.now)());
      }
      let index = 0;
      const slice = () => {
        if (epoch !== this.scanEpoch) return;
        const start = performance.now(),
          time = (this.options.now ?? Date.now)();
        do {
          const g = request.games[index++];
          if (g) {
            const current = this.current(g.id);
            if (current) this.refresh(current, time);
          }
        } while (index < request.games.length && performance.now() - start < 6);
        this.finish();
        if (index < request.games.length) this.scheduled = setTimeout(slice, 0);
        else {
          this.scheduled = undefined;
          if (this.request) this.scheduled = setTimeout(begin, 40);
        }
      };
      slice();
    };
    this.scheduled = setTimeout(begin, 40);
  }
  private finish() {
    if (this.idle && this.pending.size) {
      this.completed = 0;
      this.idle = false;
    }
    this.pump();
    this.publish();
  }
  private refresh(game: Game, time: number) {
    const policy = this.options.policy?.();
    const previous = this.known.get(game.id);
    if (
      policy !== undefined &&
      previous?.game === game &&
      previous.policy === policy &&
      previous.revision === this.revision &&
      previous.expires > time
    )
      return;

    const enabled = providerIds.filter(
      (p) => this.options.enabled?.(p, game) !== false,
    );
    const enabledKey = enabled.join(",");
    const signature = JSON.stringify([
      game.title,
      steamId(game),
      copyIdentities(game),
      game.copies.map((copy) => copy.storePage),
      game.providerIds ?? {},
      game.preferredCopy,
      game.copies.map((c) => c.emulation?.retroAchievementsGameId),
    ]);
    const known = this.known.get(game.id);
    if (
      known &&
      known.signature === signature &&
      known.policy === policy &&
      known.enrichment === game.enrichment &&
      known.revision === this.revision &&
      known.enabled === enabledKey &&
      known.expires > time
    ) {
      known.game = game;
      known.policy = policy;
      return;
    }
    this.dropPending(game.id);
    let expires = Infinity;
    for (const provider of providerIds) {
      if (!enabled.includes(provider)) continue;
      if (provider === "screenscraper" && !game.copies.some((c) => c.emulation))
        continue;
      if (
        provider === "stores" &&
        !game.copies.some(
          (c) => c.source === "gog" && /^[1-9]\d*$/.test(c.sourceId),
        )
      )
        continue;
      if (
        provider === "hltb" &&
        !game.providerIds?.hltb &&
        !currentResult(game, "pcgw")
      )
        continue;
      const fingerprint = identity(
        game,
        provider,
        this.providerRevision(provider),
      );
      const prior = game.enrichment?.[provider];
      if (prior?.fingerprint === fingerprint && prior.nextCheckAt > time) {
        expires = Math.min(expires, prior.nextCheckAt);
        continue;
      }
      const key = JSON.stringify([
        game.id,
        provider,
        fingerprint,
        this.generation,
      ]);
      if (this.active.has(key)) continue;
      const job: Job = {
        game,
        provider,
        fingerprint,
        key,
        generation: this.generation,
      };
      this.pending.set(key, job);
      const bucket = this.buckets.get(provider) ?? new Map<string, Job>();
      bucket.set(key, job);
      this.buckets.set(provider, bucket);
      const keys = this.pendingByGame.get(game.id) ?? new Set<string>();
      keys.add(key);
      this.pendingByGame.set(game.id, keys);
    }
    this.known.set(game.id, {
      game,
      policy,
      signature,
      enrichment: game.enrichment,
      revision: this.revision,
      enabled: enabledKey,
      expires,
    });
  }
  reconcile(games: Game[], revision = 0, priority: string | null = null) {
    this.scanEpoch++;
    clearTimeout(this.scheduled);
    this.scheduled = undefined;
    this.request = undefined;
    this.enabled = true;
    this.revision = revision;
    this.priority = priority;
    const time = (this.options.now ?? Date.now)();
    const ids = new Set(games.map((g) => g.id));
    for (const id of this.known.keys())
      if (!ids.has(id)) {
        this.dropPending(id);
        this.known.delete(id);
      }
    for (const game of games) this.refresh(game, time);
    this.finish();
  }
  private dropPending(id: string) {
    for (const key of this.pendingByGame.get(id) ?? []) {
      const job = this.pending.get(key);
      if (job) this.buckets.get(job.provider)?.delete(key);
      this.pending.delete(key);
    }
    this.pendingByGame.delete(id);
  }
  clear() {
    this.scanEpoch++;
    clearTimeout(this.scheduled);
    this.scheduled = undefined;
    this.request = undefined;
    this.buckets.clear();
    this.enabled = false;
    this.generation++;
    this.pending.clear();
    this.pendingByGame.clear();
    this.known.clear();
    this.completed = 0;
    this.idle = true;
    this.publish();
  }
  private publish() {
    const games = this.options.getGames();
    const active = [...this.jobs.values()]
      .filter((j) => {
        const current = this.current(j.game.id);
        return (
          this.enabled &&
          j.generation === this.generation &&
          !!current &&
          this.options.enabled?.(j.provider, current) !== false &&
          identity(current, j.provider, this.providerRevision(j.provider)) ===
            j.fingerprint
        );
      })
      .map((j) => ({ title: j.game.title, provider: j.provider }));
    const progress = {
      completed: this.completed,
      queued: this.pending.size,
      running: active.length,
      total: this.completed + this.pending.size + active.length,
      active,
    };
    if (!progress.running && !progress.queued) this.idle = true;
    const key = JSON.stringify(progress);
    if (key !== this.lastProgress) {
      this.lastProgress = key;
      this.options.onProgress?.(progress);
    }
  }
  private pump() {
    while (
      this.enabled &&
      this.running < (this.options.concurrency ?? 2) &&
      this.pending.size
    ) {
      const busy = new Map<ProviderId, number>();
      for (const j of this.jobs.values())
        busy.set(j.provider, (busy.get(j.provider) ?? 0) + 1);
      let candidate: Job | undefined;
      const choose = (job: Job) => {
        if (
          (busy.get(job.provider) ?? 0) >=
          (this.options.perProviderConcurrency ?? Infinity)
        )
          return;
        const rank = (j: Job) => [
          Number(j.game.id !== this.priority),
          Number(
            !!this.options.order &&
              (this.options.order(j.provider) ?? 0) <= 3 &&
              (busy.get(j.provider) ?? 0) > 0,
          ),
          this.options.order?.(j.provider) ?? 0,
        ];
        if (!candidate) {
          candidate = job;
          return;
        }
        const a = rank(job),
          b = rank(candidate);
        for (let i = 0; i < a.length; i++) {
          if (a[i] < b[i]) {
            candidate = job;
            return;
          }
          if (a[i] > b[i]) return;
        }
      };
      for (const bucket of this.buckets.values()) {
        const job = bucket.values().next().value;
        if (job) choose(job);
      }
      for (const key of this.priority
        ? (this.pendingByGame.get(this.priority) ?? [])
        : []) {
        const job = this.pending.get(key);
        if (job) choose(job);
      }
      if (!candidate) break;
      const job = candidate,
        key = job.key;
      this.buckets.get(job.provider)?.delete(key);
      this.pending.delete(key);
      this.pendingByGame.get(job.game.id)?.delete(key);
      const current = this.current(job.game.id);
      if (
        !current ||
        this.options.enabled?.(job.provider, current) === false ||
        identity(current, job.provider, this.providerRevision(job.provider)) !==
          job.fingerprint
      )
        continue;
      job.game = JSON.parse(JSON.stringify(current));
      this.active.add(key);
      this.jobs.set(key, job);
      this.running++;
      void this.run(job);
    }
  }
  private async run(job: Job) {
    let result: ProviderResult;
    try {
      result = await this.options.fetch(job.game, job.provider);
    } catch {
      const time = (this.options.now ?? Date.now)();
      result = {
        status: "unavailable",
        checkedAt: time,
        nextCheckAt: time + 15 * 60_000,
        message: "Provider temporarily unavailable. A retry is scheduled.",
      };
    }
    const current = this.current(job.game.id);
    if (
      this.enabled &&
      job.generation === this.generation &&
      current &&
      this.options.enabled?.(job.provider, current) !== false &&
      identity(current, job.provider, this.providerRevision(job.provider)) ===
        job.fingerprint
    ) {
      const previous = current.enrichment?.[job.provider];
      const retained =
        result.status === "unavailable" &&
        previous?.fingerprint === job.fingerprint
          ? previous
          : undefined;
      this.options.apply(current.id, job.provider, {
        ...result,
        data: result.data ?? retained?.data,
        dataCheckedAt: result.data
          ? result.checkedAt
          : (retained?.dataCheckedAt ?? retained?.checkedAt),
        fingerprint: job.fingerprint,
      });
    }
    this.active.delete(job.key);
    this.jobs.delete(job.key);
    if (job.generation === this.generation && this.enabled) this.completed++;
    this.running--;
    if (this.enabled) {
      const updated = this.current(job.game.id);
      if (updated) this.refresh(updated, (this.options.now ?? Date.now)());
      this.finish();
    }
  }
}
