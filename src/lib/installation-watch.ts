import type { Copy, Game } from "./types";

const supported = new Set(["steam", "epic", "gog", "itch", "amazon"]);
const key = (gameId: string, copyId: string) =>
  JSON.stringify([gameId, copyId]);
const identity = (copy: Copy) =>
  JSON.stringify([
    copy.source,
    copy.sourceId,
    copy.installPath,
    copy.executable,
  ]);
export interface InstallWatch {
  gameId: string;
  copyId: string;
  source: string;
  sourceId: string;
  fingerprint: string;
  addedAt: number;
  startedAt: number;
}

// Session-only evidence checks. No startup scans, download estimates, or timer per game.
export class InstallationWatch {
  private jobs = new Map<string, InstallWatch>();
  start(game: Game, copy: Copy, now = Date.now()) {
    if (copy.installed || !supported.has(copy.source)) return;
    this.jobs.set(key(game.id, copy.id), {
      gameId: game.id,
      copyId: copy.id,
      source: copy.source,
      sourceId: copy.sourceId,
      fingerprint: identity(copy),
      addedAt: game.addedAt,
      startedAt: now,
    });
  }
  clear() {
    this.jobs.clear();
  }
  pending(games: Game[], now = Date.now()) {
    if (!this.jobs.size) return [];
    const index = new Map(games.map((g) => [g.id, g]));
    for (const [key, job] of this.jobs) {
      const game = index.get(job.gameId),
        copy = game?.copies.find((c) => c.id === job.copyId);
      if (
        !game ||
        !copy ||
        copy.installed ||
        game.addedAt !== job.addedAt ||
        identity(copy) !== job.fingerprint ||
        now - job.startedAt >= 2 * 60 * 60_000
      )
        this.jobs.delete(key);
    }
    return [...this.jobs.values()];
  }
  apply(
    games: Game[],
    discovered: Game[],
    requested: InstallWatch[],
    now = Date.now(),
  ) {
    this.pending(games, now);
    const evidence = new Map(
      discovered.flatMap((g) =>
        g.copies
          .filter((c) => c.installed && c.available !== false)
          .map((c) => [key(c.source, c.sourceId), c] as const),
      ),
    );
    const updates = new Map<string, Copy>();
    for (const job of requested) {
      if (this.jobs.get(key(job.gameId, job.copyId)) !== job) continue;
      const found = evidence.get(key(job.source, job.sourceId));
      if (!found) continue;
      updates.set(key(job.gameId, job.copyId), found);
      this.jobs.delete(key(job.gameId, job.copyId));
    }
    if (!updates.size) return games;
    let changed = false;
    const result = games.map((game) => {
      let updated = false;
      const copies = game.copies.map((copy) => {
        const found = updates.get(key(game.id, copy.id));
        if (!found) return copy;
        changed = updated = true;
        return {
          ...copy,
          installed: true,
          available: true,
          installPath: found.installPath,
          executable: found.executable ?? copy.executable,
          installSizeBytes: found.installSizeBytes,
          launchUrl: found.launchUrl ?? copy.launchUrl,
        };
      });
      return updated ? { ...game, copies } : game;
    });
    return changed ? result : games;
  }
}
