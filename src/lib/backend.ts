import type { HistorySync, LaunchResult } from "./play-activity";
import {
  allowedGames,
  blacklistKeys,
  recoveryGame,
  expandBlacklistTargets,
  type BanishedGame,
  type BanishResult,
} from "./banishment";
import { writable } from "svelte/store";
import { ArtworkCatalog, libraryCoverTargets } from "./artwork-catalog";
import { listen } from "@tauri-apps/api/event";
import { currentResult } from "./enrichment-identity";
import { invoke, isTauri, convertFileSrc } from "@tauri-apps/api/core";
import {
  ArtworkLoader,
  decodeArtwork,
  type ArtworkRequest,
} from "./artwork-loader";
import type { ThemeId } from "./themes";
import type {
  EmulatorProfile,
  EmulatedGameInput,
  ConsoleId,
} from "./emulation";
import type { Copy } from "./types";
import {
  defaultSettings,
  type Library,
  type Settings,
  type ScanReport,
  type Game,
  type StoreMetadata,
  type ProviderId,
  type ProviderResult,
  type ProviderStatus,
  type ProviderConfigInput,
  type AccountStatus,
  type AccountSource,
  type AccountSync,
  type LoginTicket,
  type LibraryFolders,
} from "./types";
export const desktop = isTauri();
export async function validateEmulatorProfile(
  profile: EmulatorProfile,
): Promise<void> {
  if (!desktop) throw new Error("Configure emulators in the desktop app.");
  await invoke("validate_emulator_profile", { profile });
}
export async function chooseEmulatedFile(
  platform: ConsoleId,
): Promise<string | null> {
  return desktop ? invoke("choose_emulated_file", { platform }) : null;
}
export async function chooseEmulatorCore(): Promise<string | null> {
  return desktop ? invoke("choose_emulator_core") : null;
}
export async function addEmulated(input: EmulatedGameInput): Promise<Game> {
  if (!desktop) throw new Error("Add emulated games in the desktop app.");
  return invoke("add_emulated", { input });
}
export async function updateEmulated(
  gameId: string,
  copyId: string,
  profileId: string,
  gamePath: string,
): Promise<Copy> {
  return invoke("update_emulated", { gameId, copyId, profileId, gamePath });
}
export async function accountStatus(): Promise<AccountStatus[]> {
  return desktop ? invoke("account_status") : [];
}
export async function beginAccountLogin(
  source: AccountSource,
): Promise<LoginTicket> {
  if (!desktop) throw new Error("Connect accounts in the desktop app.");
  return invoke("begin_account_login", { source });
}
export async function reopenAccountLogin(
  source: AccountSource,
  id: string,
): Promise<void> {
  return invoke("reopen_account_login", { source, id });
}
export async function cancelAccountLogin(
  source: AccountSource,
  id: string,
): Promise<void> {
  if (desktop) return invoke("cancel_account_login", { source, id });
}
export async function finishAccountLogin(
  source: AccountSource,
  id: string,
  input: string,
): Promise<AccountStatus> {
  return invoke("finish_account_login", { source, id, input });
}
export async function connectSteamAccount(
  steamId: string,
  apiKey: string,
): Promise<AccountSync> {
  return invoke("connect_steam_account", { steamId, apiKey });
}
export async function connectItchAccount(apiKey: string): Promise<AccountSync> {
  if (!desktop) throw new Error("Connect accounts in the desktop app.");
  return invoke("connect_itch_account", { apiKey });
}
export async function disconnectAccount(
  source: AccountSource,
): Promise<AccountStatus> {
  return invoke("disconnect_account", { source });
}
export async function syncAccount(
  source: AccountSource,
  revision: number,
): Promise<AccountSync> {
  return invoke("sync_account", { source, revision });
}
const storageKey = "sloplauncher.preview.v1";
export async function loadLibrary(): Promise<Library> {
  if (desktop) return invoke("load_library");
  const saved = localStorage.getItem(storageKey);
  if (saved) {
    try {
      return JSON.parse(saved);
    } catch {
      /* recover to empty preview */
    }
  }
  return { games: [], settings: { ...defaultSettings } };
}
export async function providerStatus(): Promise<ProviderStatus> {
  return desktop
    ? invoke("provider_status")
    : { revision: 0, steamGridDbConfigured: false, igdbConfigured: false };
}
export async function saveProviderConfig(
  input: ProviderConfigInput,
): Promise<ProviderStatus> {
  if (!desktop) throw new Error("Configure providers in the desktop app.");
  return invoke("save_provider_config", { input });
}
export async function enrichProvider(
  game: Game,
  provider: ProviderId,
  accountRevision?: number,
): Promise<ProviderResult> {
  if (provider.startsWith("reviews-") && !currentResult(game, "pcgw")) {
    game = { ...game, enrichment: { ...game.enrichment, pcgw: undefined } };
  }
  return invoke("enrich_provider", { game, provider, accountRevision });
}
export async function testProvider(provider: string): Promise<string> {
  return invoke("test_provider", { provider });
}
export async function openResource(url: string): Promise<void> {
  if (desktop) await invoke("open_resource", { url });
  else window.open(url, "_blank", "noopener,noreferrer");
}
export async function checkInstallations(
  copies: { gameId: string; copyId: string }[],
): Promise<{ gameId: string; copy: Copy }[]> {
  return invoke("check_installations", { copies });
}
export interface UninstallPlan {
  source?: string;
  installPath?: string;
  title: string;
  destination: string;
  message: string;
  token: string;
}
export async function planUninstall(
  gameId: string,
  copyId: string,
): Promise<UninstallPlan> {
  return invoke("plan_uninstall", { gameId, copyId });
}
export async function uninstallGame(
  gameId: string,
  copyId: string,
  token: string,
): Promise<string> {
  return invoke("uninstall_game", { gameId, copyId, token });
}
export async function browseGameFiles(
  gameId: string,
  copyId: string,
): Promise<void> {
  if (!desktop) throw new Error("Browse game files in the desktop app.");
  await invoke("browse_game_files", { gameId, copyId });
}
export async function saveEnrichment(games: Game[]) {
  if (desktop) await invoke("save_enrichment", { games });
}
export interface GameFlagsUpdate {
  id: string;
  favorite?: boolean;
  hidden?: boolean;
  favoriteUpdatedAt?: number;
  hiddenUpdatedAt?: number;
}
export async function saveGameFlags(updates: GameFlagsUpdate[]) {
  if (desktop) await invoke("save_game_flags", { updates });
  else {
    const library = await loadLibrary();
    const byId = new Map(updates.map((update) => [update.id, update]));
    library.games = library.games.map((game) => {
      const update = byId.get(game.id);
      return update ? { ...game, ...update } : game;
    });
    localStorage.setItem(storageKey, JSON.stringify(library));
  }
}
export async function saveSettings(settings: Settings) {
  if (desktop) await invoke("save_settings", { settings });
  else {
    const library = await loadLibrary();
    library.settings = settings;
    localStorage.setItem(storageKey, JSON.stringify(library));
  }
}
export async function saveLibrary(library: Library) {
  if (!desktop)
    library = {
      ...library,
      games: allowedGames(library.games, await listBanished()),
    };
  if (desktop) await invoke("save_library", { library });
  else localStorage.setItem(storageKey, JSON.stringify(library));
}
export async function saveTheme(theme: ThemeId) {
  if (desktop) await invoke("save_theme", { theme });
  else {
    const library = await loadLibrary();
    library.settings.theme = theme;
    localStorage.setItem(storageKey, JSON.stringify(library));
  }
}
export async function scanInstalled(
  libraryFolders: LibraryFolders = {},
  sources?: string[],
): Promise<ScanReport> {
  if (!desktop)
    throw new Error("Local game discovery is available in the desktop app.");
  return invoke("scan_installed", { libraryFolders, sources });
}
export async function chooseLibraryFolder(): Promise<string | null> {
  return desktop ? invoke("choose_library_folder") : null;
}
export async function chooseExecutable(): Promise<string | null> {
  return desktop ? invoke("choose_executable") : null;
}
export async function launchGame(
  gameId: string,
  copyId: string,
): Promise<LaunchResult> {
  if (!desktop) throw new Error("Open the desktop app to launch your games.");
  return invoke("launch_game", { gameId, copyId });
}
export async function steamMetadata(appId: number): Promise<StoreMetadata> {
  if (!desktop)
    throw new Error("Metadata lookup is available in the desktop app.");
  return invoke("steam_metadata", { appId });
}
export async function searchMetadata(
  query: string,
): Promise<{ appId: number; title: string }[]> {
  if (!desktop)
    throw new Error("Game catalog search is available in the desktop app.");
  return invoke("search_metadata", { query });
}
export async function steamArtwork(
  appIds: number[],
): Promise<{ appId: number; cover: string; hero: string }[]> {
  return desktop ? invoke("steam_artwork", { appIds }) : [];
}
export async function manualGame(
  title: string,
  executable: string,
  steamId?: number,
): Promise<Game> {
  if (desktop) return invoke("add_manual", { title, executable, steamId });
  throw new Error("Add installed games from the desktop app.");
}
const artworkLoader: ArtworkLoader = new ArtworkLoader({
  resolve: async (url, thumbnail) => {
    const prepared = thumbnail === 256 && artworkCatalog.peek(url);
    if (prepared) return prepared;
    if (!desktop) return url;
    const source = artworkCatalog.source(url);
    const path = convertFileSrc(
      await invoke<string>("cached_artwork", {
        url: source,
        thumbnail: thumbnail || null,
      }),
    );
    // Browsing and background work populate the same persistent thumbnail index.
    if (thumbnail === 256) artworkCatalog.remember(url, path, source);
    return path;
  },
  decode: decodeArtwork,
});
if (typeof window !== "undefined")
  window.addEventListener("online", () => artworkLoader.reconnect());
export const artworkCatalogState = writable({
  total: 0,
  prepared: 0,
  working: false,
  retrying: 0,
  pending: 0,
  nextRetryAt: undefined as number | undefined,
});
const artworkCatalog: ArtworkCatalog = new ArtworkCatalog({
  lookup: async (urls) =>
    (await invoke<[string, string][]>("artwork_catalog", { urls })).map(
      ([url, path]): [string, string] => [url, convertFileSrc(path)],
    ),
  prepare: async (url) =>
    convertFileSrc(
      await invoke<string>("cached_artwork", { url, thumbnail: 256 }),
    ),
  foregroundBusy: () =>
    artworkLoader.stats().active > 0 || artworkLoader.stats().queued > 0,
  changed: () => artworkCatalogState.set(artworkCatalog.stats()),
});
export const setLibraryArtwork = (games: Game[]) => {
  if (!desktop) return;
  const targets = libraryCoverTargets(games);
  artworkCatalog.reconcile([...targets.keys()], targets);
};
export const invalidatePreparedArtwork = (url: string) =>
  artworkCatalog.invalidate(url);
if (desktop)
  setInterval(() => {
    void artworkCatalog.step();
  }, 250);
export const peekArtwork = (url: string, thumbnail = 0) =>
  artworkLoader.peek(url, thumbnail) ??
  (thumbnail > 0 ? artworkCatalog.peek(url) : undefined);
export const acquireArtwork = (url: string, thumbnail = 0, priority = 0) =>
  artworkLoader.acquire({ url, thumbnail, priority });
export const retryArtwork = (url: string, thumbnail = 0) =>
  artworkLoader.retry(url, thumbnail);
export const setArtworkWindow = (owner: unknown, requests: ArtworkRequest[]) =>
  artworkLoader.setWindow(owner, requests);
export const clearArtworkWindow = (owner: unknown) =>
  artworkLoader.clearWindow(owner);
export const artworkStats = () => artworkLoader.stats();

export async function listenArtworkDownloads(update: (delta: number) => void) {
  if (!desktop) return () => {};
  return listen<number>("artwork-download", (event) => {
    if (event.payload === 1 || event.payload === -1) update(event.payload);
  });
}

const banishedKey = "sloplauncher.banished.v1";
export async function listBanished(): Promise<BanishedGame[]> {
  return desktop
    ? invoke("list_banished")
    : JSON.parse(localStorage.getItem(banishedKey) ?? "[]");
}
export async function banishGames(ids: string[]): Promise<BanishResult> {
  if (desktop) return invoke("banish_games", { ids });
  const library = await loadLibrary(),
    targets = expandBlacklistTargets(library.games, ids);
  const banished = [
    ...(await listBanished()),
    ...library.games
      .filter((g) => targets.has(g.id))
      .map((g) => ({
        id: g.id,
        title: g.title,
        banishedAt: Date.now(),
        keys: blacklistKeys(g),
        game: recoveryGame(g),
        artworkHashes: [],
      })),
  ];
  library.games = library.games.filter((g) => !targets.has(g.id));
  library.settings.collections = library.settings.collections?.map((c) => ({
    ...c,
    gameIds: c.gameIds.filter((id) => !targets.has(id)),
  }));
  localStorage.setItem(banishedKey, JSON.stringify(banished));
  localStorage.setItem(storageKey, JSON.stringify(library));
  return { library, banished };
}
export async function unbanishGames(ids: string[]): Promise<BanishResult> {
  if (desktop) return invoke("unbanish_games", { ids });
  const all = await listBanished(),
    library = await loadLibrary();
  const targets = expandBlacklistTargets(
    all.map((b) => ({
      ...b.game,
      metadataId:
        Number(b.keys.find((k) => k.startsWith("match:steam:"))?.slice(12)) ||
        undefined,
    })),
    ids,
  );
  const restored = all.filter((b) => targets.has(b.id)),
    banished = all.filter((b) => !targets.has(b.id));
  library.games.push(...restored.map((b) => b.game));
  localStorage.setItem(banishedKey, JSON.stringify(banished));
  localStorage.setItem(storageKey, JSON.stringify(library));
  return { library, banished };
}
export function forgetArtworkMemory() {
  artworkLoader.clear();
  artworkCatalog.reconcile([]);
}

export async function syncSteamHistory(force = false): Promise<HistorySync> {
  return invoke("sync_steam_history", { force });
}
