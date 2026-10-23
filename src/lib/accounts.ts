import type {
  AccountSource,
  AccountStatus,
  AccountSync,
  LoginTicket,
} from "./types";
export const accountSources: AccountSource[] = [
  "steam",
  "epic",
  "amazon",
  "gog",
  "itch",
];
export interface AccountState extends AccountStatus {
  phase:
    | "idle"
    | "opening"
    | "signing-in"
    | "connecting"
    | "syncing"
    | "disconnecting";
  ticket?: LoginTicket;
}
export function initialAccounts(): AccountState[] {
  return accountSources.map((source) => ({
    source,
    connected: false,
    revision: 0,
    reconnectRequired: false,
    phase: "idle",
  }));
}
export interface AccountApi {
  accountStatus(): Promise<AccountStatus[]>;
  beginAccountLogin(source: AccountSource): Promise<LoginTicket>;
  reopenAccountLogin(source: AccountSource, id: string): Promise<void>;
  cancelAccountLogin(source: AccountSource, id: string): Promise<void>;
  finishAccountLogin(
    source: AccountSource,
    id: string,
    input: string,
  ): Promise<AccountStatus>;
  connectSteamAccount(steamId: string, apiKey: string): Promise<AccountSync>;
  connectItchAccount(apiKey: string): Promise<AccountSync>;
  disconnectAccount(source: AccountSource): Promise<AccountStatus>;
  syncAccount(source: AccountSource, revision: number): Promise<AccountSync>;
}
interface Hooks {
  changed(states: AccountState[]): void;
  imported(result: AccountSync, automatic: boolean): void;
  enabled(): boolean;
  copyIds?: () => Set<string>;
  now?: () => number;
}
export class AccountManager {
  private states = initialAccounts();
  private epochs = new Map<AccountSource, number>();
  private attempts = new Map<AccountSource, number>();
  private active = new Set<AccountSource>();
  private alive = true;
  constructor(
    private api: AccountApi,
    private hooks: Hooks,
  ) {}
  private time() {
    return (this.hooks.now ?? Date.now)();
  }
  private state(s: AccountSource) {
    return this.states.find((a) => a.source === s)!;
  }
  private epoch(s: AccountSource) {
    return this.epochs.get(s) ?? 0;
  }
  private bump(s: AccountSource) {
    const n = this.epoch(s) + 1;
    this.epochs.set(s, n);
    return n;
  }
  private current(s: AccountSource, n: number) {
    return this.alive && this.epoch(s) === n;
  }
  private patch(s: AccountSource, update: Partial<AccountState>) {
    this.states = this.states.map((a) =>
      a.source === s ? { ...a, ...update } : a,
    );
    if (this.alive) this.hooks.changed(this.states.map((a) => ({ ...a })));
  }
  private deliver(
    result: AccountSync,
    before: Set<string> | undefined,
    automatic: boolean,
  ) {
    const current = this.hooks.copyIds?.();
    const safe =
      before && current
        ? {
            ...result,
            games: result.games
              .map((g) => ({
                ...g,
                copies: g.copies.filter(
                  (c) => !before.has(c.id) || current.has(c.id),
                ),
              }))
              .filter((g) => g.copies.length),
          }
        : result;
    this.hooks.imported(safe, automatic);
  }
  async load() {
    const epochs = new Map(accountSources.map((s) => [s, this.epoch(s)]));
    const statuses = await this.api.accountStatus();
    for (const status of statuses)
      if (
        accountSources.includes(status.source) &&
        this.current(status.source, epochs.get(status.source)!)
      )
        this.patch(status.source, status);
  }
  async begin(source: AccountSource) {
    const n = this.bump(source);
    this.patch(source, {
      phase: "opening",
      error: undefined,
      ticket: undefined,
    });
    try {
      const ticket = await this.api.beginAccountLogin(source);
      if (this.current(source, n))
        this.patch(source, { ticket, phase: "signing-in" });
      else await this.api.cancelAccountLogin(source, ticket.id);
    } catch (error) {
      if (this.current(source, n))
        this.patch(source, { error: String(error), phase: "idle" });
    }
  }
  async reopen(source: AccountSource) {
    const ticket = this.state(source).ticket;
    if (!ticket) return this.begin(source);
    try {
      await this.api.reopenAccountLogin(source, ticket.id);
    } catch (e) {
      this.patch(source, { error: String(e) });
    }
  }
  async cancel(source: AccountSource) {
    const ticket = this.state(source).ticket;
    this.bump(source);
    this.patch(source, { phase: "idle", ticket: undefined, error: undefined });
    if (ticket)
      await this.api.cancelAccountLogin(source, ticket.id).catch(() => {});
  }
  async finish(source: AccountSource, input: string): Promise<boolean> {
    const ticket = this.state(source).ticket;
    if (!ticket) {
      this.patch(source, { error: "Start sign-in in your browser first." });
      return false;
    }
    if (ticket.expiresAt <= this.time()) {
      this.patch(source, {
        error: "Sign-in timed out. Start again.",
        ticket: undefined,
        phase: "idle",
      });
      return false;
    }
    const n = this.epoch(source);
    this.patch(source, { phase: "connecting", error: undefined });
    try {
      const status = await this.api.finishAccountLogin(
        source,
        ticket.id,
        input,
      );
      if (!this.current(source, n)) return false;
      this.patch(source, { ...status, phase: "idle", ticket: undefined });
      await this.sync(source);
      return true;
    } catch (e) {
      if (this.current(source, n))
        this.patch(source, {
          phase: "idle",
          ticket: undefined,
          error: String(e),
        });
      return false;
    }
  }
  async steam(steamId: string, key: string): Promise<boolean> {
    return this.keyed("steam", () =>
      this.api.connectSteamAccount(steamId, key),
    );
  }
  async itch(key: string): Promise<boolean> {
    return this.keyed("itch", () => this.api.connectItchAccount(key));
  }
  private async keyed(
    source: AccountSource,
    request: () => Promise<AccountSync>,
  ): Promise<boolean> {
    const n = this.bump(source);
    const copies = this.hooks.copyIds?.();
    this.patch(source, { phase: "connecting", error: undefined });
    try {
      const result = await request();
      if (!this.current(source, n)) return false;
      this.patch(source, { ...result.account, phase: "idle" });
      this.attempts.set(source, this.time());
      if (this.hooks.enabled()) this.deliver(result, copies, false);
      return true;
    } catch (e) {
      if (this.current(source, n))
        this.patch(source, { phase: "idle", error: String(e) });
      return false;
    }
  }
  async sync(source: AccountSource, automatic = false) {
    const a = this.state(source);
    const n = this.epoch(source);
    if (
      !this.hooks.enabled() ||
      !a.connected ||
      a.reconnectRequired ||
      a.phase !== "idle" ||
      this.active.has(source)
    )
      return;
    this.active.add(source);
    this.attempts.set(source, this.time());
    const copies = this.hooks.copyIds?.();
    this.patch(source, { phase: "syncing", error: undefined });
    try {
      const result = await this.api.syncAccount(source, a.revision);
      if (
        !this.current(source, n) ||
        !this.hooks.enabled() ||
        result.revision !== this.state(source).revision
      )
        return;
      this.patch(source, { ...result.account, phase: "idle" });
      this.deliver(result, copies, automatic);
    } catch (e) {
      if (this.current(source, n)) {
        this.patch(source, { phase: "idle", error: String(e) });
        // Refresh safe status flags (including expired sign-in), never tokens.
        await this.load().catch(() => {});
        if (this.current(source, n)) this.patch(source, { error: String(e) });
      }
    } finally {
      this.active.delete(source);
      if (this.current(source, n) && this.state(source).phase === "syncing")
        this.patch(source, { phase: "idle" });
    }
  }
  async disconnect(source: AccountSource) {
    const n = this.bump(source);
    this.patch(source, {
      phase: "disconnecting",
      ticket: undefined,
      error: undefined,
    });
    try {
      const status = await this.api.disconnectAccount(source);
      if (this.current(source, n))
        this.patch(source, { ...status, phase: "idle" });
    } catch (e) {
      if (this.current(source, n))
        this.patch(source, { phase: "idle", error: String(e) });
    }
  }
  async syncAll(automatic = false) {
    if (!this.hooks.enabled()) return;
    const due = this.states.filter(
      (a) =>
        a.connected &&
        !a.reconnectRequired &&
        a.phase === "idle" &&
        (!automatic ||
          this.time() -
            Math.max(a.lastSync ?? 0, this.attempts.get(a.source) ?? 0) >=
            15 * 60_000),
    );
    for (let i = 0; i < due.length; i += 2)
      await Promise.allSettled(
        due.slice(i, i + 2).map((a) => this.sync(a.source, automatic)),
      );
  }
  dispose() {
    this.alive = false;
    for (const s of accountSources) this.bump(s);
  }
}
