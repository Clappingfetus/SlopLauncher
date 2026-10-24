import { describe, it, expect, vi } from "vitest";
import { AccountManager, initialAccounts, type AccountApi } from "./accounts";
import { mergeOwnedLibrary, mergeDiscovered, filterGames } from "./library";
import { demoGames } from "./demo";
import type { AccountSource, AccountStatus, AccountSync, Game } from "./types";
const tick = async () => {
  for (let i = 0; i < 30; i++) await Promise.resolve();
};
const account = (source: AccountSource, revision = 1): AccountStatus => ({
  source,
  revision,
  connected: true,
  reconnectRequired: false,
  accountId: `${source}-account`,
  displayName: `${source} user`,
  ownedCount: 1,
});
const owned = (
  source: AccountSource,
  id = "new",
  title = "Newly owned game",
): Game => ({
  ...demoGames()[0],
  id: `${source}:${id}`,
  title,
  copies: [
    {
      id: `${source}:${id}`,
      source,
      sourceId: id,
      installed: false,
      ownership: { accountId: `${source}-account`, verifiedAt: 10 },
    },
  ],
  favorite: false,
  notes: "",
  enrichment: undefined,
});
const report = (
  source: AccountSource,
  games = [owned(source)],
  revision = 1,
): AccountSync => ({
  source,
  revision,
  account: account(source, revision),
  games,
  messages: [],
});
function harness(statuses = [account("epic")]) {
  let states = initialAccounts(),
    enabled = true,
    time = 2_000_000;
  let games: Game[] = [];
  const api: AccountApi = {
    accountStatus: vi.fn(async () => statuses),
    beginAccountLogin: vi.fn(async () => ({
      id: "one-time-session",
      expiresAt: time + 600000,
    })),
    reopenAccountLogin: vi.fn(async () => {}),
    cancelAccountLogin: vi.fn(async () => {}),
    finishAccountLogin: vi.fn(async (s) => account(s)),
    connectSteamAccount: vi.fn(async () => report("steam")),
    connectItchAccount: vi.fn(async () => report("itch")),
    disconnectAccount: vi.fn(async (s) => ({
      ...account(s, 2),
      connected: false,
      accountId: undefined,
    })),
    syncAccount: vi.fn(async (s) => report(s)),
  };
  const imported = vi.fn((r: AccountSync) => {
    games = mergeOwnedLibrary(games, r.games, r.source);
  });
  const manager = new AccountManager(api, {
    changed: (s) => (states = s),
    enabled: () => enabled,
    now: () => time,
    copyIds: () => new Set(games.flatMap((g) => g.copies.map((c) => c.id))),
    imported,
  });
  return {
    api,
    manager,
    imported,
    get states() {
      return states;
    },
    get games() {
      return games;
    },
    set games(value) {
      games = value;
    },
    set enabled(value: boolean) {
      enabled = value;
    },
    advance(ms: number) {
      time += ms;
    },
  };
}
describe("account sync lifecycle", () => {
  it("connects itch securely, imports later additions, and keeps installed copies and personal edits", async () => {
    const h = harness([]);
    expect(await h.manager.itch("fixture-only-itch-key")).toBe(true);
    const installed = owned("itch");
    installed.copies[0].installed = true;
    installed.copies[0].installPath = "C:\\Games\\itch-game";
    h.games = mergeDiscovered(h.games, [installed]);
    h.games[0].notes = "My notes";
    h.games[0].favorite = true;
    h.games[0].providerIds = { igdb: 123 };
    vi.mocked(h.api.syncAccount).mockResolvedValue(
      report("itch", [owned("itch"), owned("itch", "future")]),
    );
    await h.manager.sync("itch");
    expect(h.api.syncAccount).toHaveBeenCalledWith("itch", 1);
    expect(h.games).toHaveLength(2);
    expect(h.games[0]).toMatchObject({
      notes: "My notes",
      favorite: true,
      providerIds: { igdb: 123 },
    });
    expect(h.games[0].copies[0]).toMatchObject({
      installed: true,
      installPath: "C:\\Games\\itch-game",
    });
    expect(filterGames(h.games, "itch", "", false, "", "title")).toHaveLength(
      2,
    );
    expect(JSON.stringify(h.states)).not.toContain("fixture-only-itch-key");
  });
  it("discards an itch connection response after disconnect", async () => {
    const h = harness([]);
    let release!: (value: AccountSync) => void;
    vi.mocked(h.api.connectItchAccount).mockImplementation(
      () => new Promise((r) => (release = r)),
    );
    const pending = h.manager.itch("fixture-only-key");
    await h.manager.disconnect("itch");
    release(report("itch"));
    expect(await pending).toBe(false);
    expect(h.games).toHaveLength(0);
    expect(h.states.find((a) => a.source === "itch")?.connected).toBe(false);
  });
  it("imports full owned games, including later additions, using the saved revision", async () => {
    const h = harness();
    await h.manager.load();
    await h.manager.sync("epic");
    expect(h.api.syncAccount).toHaveBeenCalledWith("epic", 1);
    expect(h.games).toHaveLength(1);
    expect(h.games[0].copies[0].installed).toBe(false);
    vi.mocked(h.api.syncAccount).mockResolvedValue(
      report("epic", [
        owned("epic"),
        owned("epic", "future", "Added after initial sync"),
      ]),
    );
    await h.manager.sync("epic");
    expect(h.games.map((g) => g.title)).toContain("Added after initial sync");
  });
  it("does not overlap manual and automatic syncs for one source", async () => {
    const h = harness();
    let release!: (r: AccountSync) => void;
    vi.mocked(h.api.syncAccount).mockImplementation(
      () => new Promise((r) => (release = r)),
    );
    await h.manager.load();
    const first = h.manager.sync("epic");
    await h.manager.syncAll(true);
    await h.manager.sync("epic");
    expect(h.api.syncAccount).toHaveBeenCalledTimes(1);
    release(report("epic"));
    await first;
  });
  it("discards a result after disconnection without deleting cached games", async () => {
    const h = harness();
    h.games = [owned("epic", "cached")];
    await h.manager.load();
    let release!: (r: AccountSync) => void;
    vi.mocked(h.api.syncAccount).mockImplementation(
      () => new Promise((r) => (release = r)),
    );
    const sync = h.manager.sync("epic");
    await h.manager.disconnect("epic");
    release(report("epic"));
    await sync;
    expect(h.imported).not.toHaveBeenCalled();
    expect(h.games).toHaveLength(1);
    expect(h.states.find((a) => a.source === "epic")?.connected).toBe(false);
  });
  it("never resurrects a copy removed while a sync is downloading", async () => {
    const h = harness();
    h.games = [owned("epic", "removed")];
    await h.manager.load();
    let release!: (r: AccountSync) => void;
    vi.mocked(h.api.syncAccount).mockImplementation(
      () => new Promise((r) => (release = r)),
    );
    const sync = h.manager.sync("epic");
    h.games = [];
    release(
      report("epic", [owned("epic", "removed"), owned("epic", "future")]),
    );
    await sync;
    expect(h.games.map((g) => g.id)).toEqual(["epic:future"]);
  });
  it("does not apply work after switching to sample mode or shutting down", async () => {
    const h = harness();
    await h.manager.load();
    let release!: (r: AccountSync) => void;
    vi.mocked(h.api.syncAccount).mockImplementation(
      () => new Promise((r) => (release = r)),
    );
    const sync = h.manager.sync("epic");
    h.enabled = false;
    h.manager.dispose();
    release(report("epic"));
    await sync;
    expect(h.imported).not.toHaveBeenCalled();
  });
  it("isolates a failed store and keeps its existing games", async () => {
    const h = harness([account("epic"), account("gog")]);
    h.games = [owned("epic", "existing")];
    vi.mocked(h.api.syncAccount).mockImplementation(async (s) => {
      if (s === "epic") throw new Error("Store unavailable");
      return report(s);
    });
    await h.manager.load();
    await h.manager.syncAll();
    expect(h.games.map((g) => g.id)).toEqual(["epic:existing", "gog:new"]);
  });
  it("retries automatically after the interval and lets manual refresh run immediately", async () => {
    const h = harness();
    await h.manager.load();
    await h.manager.syncAll(true);
    await h.manager.syncAll(true);
    expect(h.api.syncAccount).toHaveBeenCalledTimes(1);
    await h.manager.sync("epic");
    expect(h.api.syncAccount).toHaveBeenCalledTimes(2);
    h.advance(3_600_001);
    await h.manager.syncAll(true);
    expect(h.api.syncAccount).toHaveBeenCalledTimes(3);
  });
  it("requires a reconnect instead of repeatedly retrying expired authentication", async () => {
    const h = harness([{ ...account("gog"), reconnectRequired: true }]);
    await h.manager.load();
    await h.manager.syncAll(true);
    expect(h.api.syncAccount).not.toHaveBeenCalled();
  });
  it("cancels a browser login that finishes opening after cancellation", async () => {
    const h = harness();
    let release!: (r: { id: string; expiresAt: number }) => void;
    vi.mocked(h.api.beginAccountLogin).mockImplementation(
      () => new Promise((r) => (release = r)),
    );
    const opening = h.manager.begin("amazon");
    await h.manager.cancel("amazon");
    release({ id: "late-login", expiresAt: 3_000_000 });
    await opening;
    expect(h.api.cancelAccountLogin).toHaveBeenCalledWith(
      "amazon",
      "late-login",
    );
    expect(h.states.find((a) => a.source === "amazon")?.ticket).toBeUndefined();
  });
  it("discards a completed login after the user cancels it", async () => {
    const h = harness();
    await h.manager.begin("gog");
    let release!: (r: AccountStatus) => void;
    vi.mocked(h.api.finishAccountLogin).mockImplementation(
      () => new Promise((r) => (release = r)),
    );
    const finishing = h.manager.finish("gog", "temporary-code");
    await h.manager.cancel("gog");
    release(account("gog"));
    expect(await finishing).toBe(false);
    expect(h.api.syncAccount).not.toHaveBeenCalled();
  });
  it("connects Steam once then syncs without passing its key through frontend storage", async () => {
    const h = harness([]);
    expect(await h.manager.steam("76561198000000000", "fixture-only-key")).toBe(
      true,
    );
    await h.manager.sync("steam");
    expect(h.api.syncAccount).toHaveBeenCalledWith("steam", 1);
    expect(JSON.stringify(h.states)).not.toContain("fixture-only-key");
  });
  it("does not let a stale status request overwrite a newer connection", async () => {
    const h = harness();
    let release!: (r: AccountStatus[]) => void;
    vi.mocked(h.api.accountStatus).mockImplementation(
      () => new Promise((r) => (release = r)),
    );
    const load = h.manager.load();
    await h.manager.steam("id", "fixture-key");
    release([account("steam", 0)]);
    await load;
    expect(h.states.find((a) => a.source === "steam")?.revision).toBe(1);
  });
});
describe("owned-library merge", () => {
  it("preserves installation evidence, launch data and edits made while fetching", () => {
    const before = owned("gog", "123");
    before.title = "My custom name";
    before.notes = "Keep this";
    before.favorite = true;
    before.status = "completed";
    before.copies[0] = {
      ...before.copies[0],
      installed: true,
      available: true,
      executable: "C:\\Games\\game.exe",
      installPath: "C:\\Games",
    };
    const result = mergeOwnedLibrary(
      [before],
      [owned("gog", "123", "Store title")],
      "gog",
    );
    expect(result[0]).toMatchObject({
      title: "My custom name",
      notes: "Keep this",
      favorite: true,
      status: "completed",
    });
    expect(result[0].copies[0]).toMatchObject({
      installed: true,
      available: true,
      executable: "C:\\Games\\game.exe",
    });
    expect(before.notes).toBe("Keep this");
  });
  it("keeps manual games and absent games when an owned response is empty", () => {
    const games = demoGames();
    expect(mergeOwnedLibrary(games, [], "amazon")).toHaveLength(games.length);
  });
  it("deduplicates repeated records and repeated imports by copy identity", () => {
    const game = owned("amazon");
    const once = mergeOwnedLibrary([], [game, game], "amazon");
    expect(once).toHaveLength(1);
    expect(mergeOwnedLibrary(once, [game], "amazon")).toHaveLength(1);
  });
  it("rejects a mixed-source result before changing anything", () => {
    const game = owned("gog");
    expect(() => mergeOwnedLibrary([game], [owned("epic")], "gog")).toThrow();
    expect(game.copies[0].source).toBe("gog");
  });
  it("replaces a temporary store title on a future successful sync", () => {
    const game = owned("epic", "123", "Epic game 123");
    expect(
      mergeOwnedLibrary(
        [game],
        [owned("epic", "123", "Proper title")],
        "epic",
      )[0].title,
    ).toBe("Proper title");
  });
});

describe("persistent automatic sync scheduling", () => {
  it("restores a failure cooldown in a new manager while allowing manual refresh", async () => {
    const saved = { ...account("epic"), nextRetryAt: 2_000_000 + 3_600_000 };
    const h = harness([saved]);
    await h.manager.load();
    await h.manager.syncAll(true);
    await h.manager.sync("epic", true);
    expect(h.api.syncAccount).not.toHaveBeenCalled();
    h.advance(3_600_000);
    await h.manager.syncAll(true);
    expect(h.api.syncAccount).toHaveBeenCalledTimes(1);
    const reopened = harness([saved]);
    await reopened.manager.load();
    await reopened.manager.sync("epic");
    expect(reopened.api.syncAccount).toHaveBeenCalledTimes(1);
  });
  it("defaults to six hours after a successful sync, even after reopening", async () => {
    const h = harness([{ ...account("gog"), lastSync: 2_000_000 }]);
    await h.manager.load();
    h.advance(3_600_000);
    await h.manager.syncAll(true);
    expect(h.api.syncAccount).not.toHaveBeenCalled();
    h.advance(5 * 3_600_000);
    await h.manager.syncAll(true);
    expect(h.api.syncAccount).toHaveBeenCalledTimes(1);
  });
  it("supports manual-only and a changed interval without disabling new imports", async () => {
    const h = harness();
    let hours = 0;
    const manager = new AccountManager(h.api, {
      changed: () => {},
      imported: h.imported,
      enabled: () => true,
      now: () => 100_000_000,
      syncHours: () => hours,
    });
    await manager.load();
    await manager.syncAll(true);
    expect(h.api.syncAccount).not.toHaveBeenCalled();
    await manager.syncAll(false);
    expect(h.imported).toHaveBeenCalledTimes(1);
    manager.dispose();
    hours = 1;
    const reopened = new AccountManager(h.api, {
      changed: () => {},
      imported: h.imported,
      enabled: () => true,
      now: () => 100_000_000,
      syncHours: () => hours,
    });
    await reopened.load();
    await reopened.syncAll(true);
    expect(h.imported).toHaveBeenCalledTimes(2);
  });
});
