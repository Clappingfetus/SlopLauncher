<script lang="ts">
  import { steamId } from "./lib/enrichment-identity";
  import ReviewRatings from "./components/ReviewRatings.svelte";
  import { needsAmazonPortraitRefresh } from "./lib/store-artwork";
  import { version as appVersion } from "../package.json";
  import { onMount, tick, untrack } from "svelte";
  import {
    Library,
    Clock3,
    Heart,
    EyeOff,
    Folder,
    Plus,
    Search,
    SlidersHorizontal,
    ChevronDown,
    ArrowLeft,
    ArrowUpRight,
    Play,
    Download,
    Settings2,
    RefreshCw,
    Check,
    CheckCircle2,
    X,
    Gamepad2,
    Link2,
    HardDrive,
    Image,
    Info,
    LoaderCircle,
    ExternalLink,
    Monitor,
    ChevronRight,
    FolderOpen,
    CheckCheck,
    Unplug,
    FlaskConical,
  } from "@lucide/svelte";
  import Cover from "./components/Cover.svelte";
  import VirtualGrid from "./components/VirtualGrid.svelte";
  import SmartGroupEditor from "./components/SmartGroupEditor.svelte";
  import { groupGames, groupOptions, validateGroups } from "./lib/grouping";
  import type { GroupMode, SmartGroup, Copy } from "./lib/types";
  import GameLinks from "./components/GameLinks.svelte";
  import { createVersionResolver, gameContextIdentity } from "./lib/versions";
  import { reconcileStoreTitles } from "./lib/store-titles";
  import {
    changeDuplicateFlag,
    reconcileDuplicateFlags,
  } from "./lib/duplicate-flags";
  import { InstallationWatch } from "./lib/installation-watch";
  import { launchAction } from "./lib/launch-action";
  import GameMenu from "./components/GameMenu.svelte";
  import ArtworkPicker from "./components/ArtworkPicker.svelte";
  import { artworkCatalogState } from "./lib/backend";
  import EnrichmentStatus from "./components/EnrichmentStatus.svelte";
  import Achievements from "./components/Achievements.svelte";
  import {
    createGapPolicy,
    gapProviderOrder,
    missingFields,
    isLibraryResource,
    libraryResourceEnabled,
  } from "./lib/gaps";
  import { artworkIdentity, selectedArtwork } from "./lib/artwork";
  import AppLogo from "./components/AppLogo.svelte";
  import ThemePicker from "./components/ThemePicker.svelte";
  import EmulatorSettings from "./components/EmulatorSettings.svelte";
  import EmulatedGameForm from "./components/EmulatedGameForm.svelte";
  import CollectionEditor from "./components/CollectionEditor.svelte";
  import GameCollections from "./components/GameCollections.svelte";
  import {
    isCollectionFilter,
    saveCollection,
    setMembership,
    withoutGame,
    collectionCounts,
    resolveCollections,
  } from "./lib/collections";
  import {
    consoleName,
    type EmulatorProfile,
    type EmulatedGameInput,
  } from "./lib/emulation";
  import { applyTheme, resolveTheme, type ThemeId } from "./lib/themes";
  import GameTags from "./components/GameTags.svelte";
  import { libraryTags, tagKey } from "./lib/tags";
  import CompletionTimes from "./components/CompletionTimes.svelte";
  import SetupWizard from "./components/SetupWizard.svelte";
  import ProviderSettings from "./components/ProviderSettings.svelte";
  import SourceConnections from "./components/SourceConnections.svelte";
  import { AccountManager, initialAccounts } from "./lib/accounts";
  import {
    EnrichmentQueue,
    presentGame,
    createGamePresenter,
    identity,
    currentResult,
    type EnrichmentProgress,
  } from "./lib/enrichment";
  import {
    type Game,
    type GameCollection,
    type Source,
    type Settings,
    type AccountSource,
    sourceNames,
    sources,
    defaultSettings,
    type ProviderStatus,
    type ArtworkAsset,
    type ArtworkChoice,
  } from "./lib/types";
  import {
    filterGames,
    sortOptions,
    mergeDiscovered,
    mergeOwnedLibrary,
    preferredCopy,
    type Filter,
  } from "./lib/library";
  import { demoGames } from "./lib/demo";
  import * as api from "./lib/backend";

  let games = $state.raw<Game[]>([]),
    settings = $state<Settings>({ ...defaultSettings }),
    demoMode = $state(false),
    ready = $state(false);
  let filter = $state<Filter>("all"),
    query = $state(""),
    installedOnly = $state(false),
    tagFilter = $state(""),
    sort = $state("title"),
    showFilters = $state(false);
  let page = $state<"library" | "sources" | "settings" | "setup">("library"),
    selectedId = $state<string | null>(null),
    busy = $state(""),
    toast = $state(""),
    toastError = $state(false),
    scanMessages = $state<string[]>([]);
  let modal = $state<
      | "add"
      | "metadata"
      | "screenshot"
      | "remove"
      | "uninstall"
      | "emulation"
      | "collection"
      | "artwork"
      | "grouping"
      | null
    >(null),
    dialog: HTMLDialogElement;
  let searchInput = $state<HTMLInputElement>();
  let addMode = $state<"pc" | "emulated">("pc");
  let addTitle = $state(""),
    addPath = $state(""),
    addSteamId = $state(""),
    metadataId = $state(""),
    formError = $state(""),
    screenshot = $state("");
  let toastTimer: ReturnType<typeof setTimeout>;
  let metadataQuery = $state(""),
    metadataResults = $state<{ appId: number; title: string }[]>([]),
    metadataSearched = $state(false);
  let persistence = Promise.resolve();
  let libraryEpoch = 0,
    metadataRequest = 0;
  let loadError = $state("");
  let collectionDraft = $state<GameCollection>({
    id: "",
    name: "",
    gameIds: [],
  });
  let groupingEditorDraft = $state<SmartGroup[] | null>(null);
  let collectionReturnToGroups = $state(false);
  let collections = $derived(demoMode ? [] : (settings.collections ?? []));
  let activeCollection = $derived(
    isCollectionFilter(filter)
      ? collections.find((c) => c.id === filter.slice(11))
      : undefined,
  );
  let browsableGames = $derived(
    settings.showHiddenGames ? games : games.filter((g) => !g.hidden),
  );
  let artworkRequests = $state<string[]>([]);
  let artworkGame = $derived(games.find((g) => g.id === selectedId));
  let providers = $state<ProviderStatus>({
    revision: 0,
    steamGridDbConfigured: false,
    igdbConfigured: false,
  });
  let enrichmentSaveTimer: ReturnType<typeof setTimeout> | undefined;
  let notesSaveTimer: ReturnType<typeof setTimeout> | undefined;
  let enrichmentProgress = $state<EnrichmentProgress>({
    completed: 0,
    queued: 0,
    running: 0,
    total: 0,
    active: [],
  });
  const resolveVersions = createVersionResolver();
  const presentCached = createGamePresenter();
  const gapEnabled = createGapPolicy(presentCached);
  let gameIndex = $derived(new Map(games.map((g) => [g.id, g])));
  const dirtyEnrichment = new Set<string>();
  let lastEnrichmentSave = 0;
  const enrichmentQueue = new EnrichmentQueue({
    concurrency: 6,
    perProviderConcurrency: 2,
    getGame: (id) => gameIndex.get(id),
    policy: () => enrichmentPolicy,
    revisionFor: (provider) =>
      provider === "achievements"
        ? (steamAchievementAccount?.revision ?? 0)
        : providers.revision,
    order: (provider) => gapProviderOrder[provider],
    onProgress: (progress) => (enrichmentProgress = progress),
    getGames: () => games,
    fetch: (game, provider) =>
      api.enrichProvider(
        $state.snapshot(game),
        provider,
        provider === "achievements"
          ? steamAchievementAccount?.revision
          : provider === "retroachievements"
            ? providers.revision
            : undefined,
      ),
    enabled: (provider, game) => {
      if (isLibraryResource(provider))
        return (
          (settings.autoFillGaps !== false ||
            ratingDetailIds.has(game.id) ||
            selectedId === game.id) &&
          libraryResourceEnabled(game, provider)
        );
      if (provider === "retroachievements")
        return (
          selectedId === game.id &&
          !!providers.retroAchievementsConfigured &&
          preferredCopy(game)?.source === "emulated"
        );
      if (provider === "achievements")
        return (
          selectedId === game.id &&
          !!steamAchievementAccount?.connected &&
          !steamAchievementAccount.reconnectRequired &&
          preferredCopy(game)?.source === "steam"
        );
      if (
        provider === "steamgriddb" &&
        modal === "artwork" &&
        selectedId === game.id &&
        artworkRequests.includes(artworkIdentity(game))
      )
        return providers.steamGridDbConfigured;
      if (modal === "metadata" && selectedId === game.id) {
        if (provider === "igdb") return providers.igdbConfigured;
        if (provider === "screenscraper")
          return !!providers.screenScraperConfigured;
      }
      return (
        settings.autoFillGaps !== false && gapEnabled(game, provider, providers)
      );
    },
    apply: (id, provider, result) => {
      games = games.map((g) =>
        g.id === id
          ? { ...g, enrichment: { ...g.enrichment, [provider]: result } }
          : g,
      );
      dirtyEnrichment.add(id);
      if (!enrichmentSaveTimer)
        enrichmentSaveTimer = setTimeout(
          flushEnrichment,
          Math.max(100, 1000 - (Date.now() - lastEnrichmentSave)),
        );
    },
  });
  let accountStates = $state(initialAccounts());
  let steamAchievementAccount = $derived(
    accountStates.find((a) => a.source === "steam"),
  );
  let artworkDownloads = $state(0);
  const accountManager = new AccountManager(api, {
    syncHours: () => settings.librarySyncHours,
    changed: (states) => (accountStates = states),
    enabled: () => api.desktop && ready && !demoMode,
    copyIds: () => new Set(games.flatMap((g) => g.copies.map((c) => c.id))),
    imported: (result, automatic) => {
      const before = games.length;
      const previousGames = games,
        previousSteamId = settings.steamId;
      games = mergeOwnedLibrary(games, result.games, result.source);
      if (result.source === "steam")
        settings.steamId = result.account.accountId ?? settings.steamId;
      if (games !== previousGames || settings.steamId !== previousSteamId)
        persist();
      const added = games.length - before;
      if (!automatic || added)
        notify(
          `${sourceNames[result.source]} synced · ${result.games.length} owned${added ? ` · ${added} added` : ""}.`,
        );
      if (result.messages.length) scanMessages = result.messages;
    },
  });
  let libraryScroll = $state(0),
    saveStatus = $state<"saved" | "saving" | "error">("saved");
  let basePresentedGames = $derived(
    games.map((g) => presentCached(g, providers.revision)),
  );
  $effect(() => {
    api.setLibraryArtwork(
      ready && !demoMode
        ? basePresentedGames
            .map((g) => g.cover)
            .filter((url): url is string => !!url)
        : [],
    );
  });
  let versionsIndex = $derived(
    resolveVersions(basePresentedGames, settings, true, providers.revision),
  );
  let ratingDetailIds = $derived(
    new Set(
      selectedId ? versionsIndex.members(selectedId).map((g) => g.id) : [],
    ),
  );
  let presentedGames = $derived(
    basePresentedGames.map(versionsIndex.withPresentation),
  );
  let selectedVersions = $derived(
    selectedId ? versionsIndex.versions(selectedId) : [],
  );
  let contextMenu = $state.raw<{
    gameId: string;
    copyId?: string;
    identity: string;
    anchor: { x: number; y: number; target?: HTMLElement };
    slot?: "cover" | "hero";
  } | null>(null);
  let artworkSlot = $state<"cover" | "hero">("cover");
  let uninstallRequest = $state<{
    gameId: string;
    copyId: string;
    plan: api.UninstallPlan;
  } | null>(null);
  let contextGame = $derived.by(() => {
    const state = contextMenu;
    if (!state) return undefined;
    const game = gameIndex.get(state.gameId);
    if (
      !game ||
      gameContextIdentity(
        game,
        game.copies.find((c) => c.id === state.copyId),
      ) !== state.identity
    )
      return undefined;
    return {
      ...versionsIndex.withPresentation(
        presentCached(game, providers.revision),
      ),
      preferredCopy: state.copyId,
    };
  });
  let hiddenCount = $derived(
    versionsIndex.collapse(presentedGames.filter((g) => g.hidden)).length,
  );
  let libraryCount = $derived(
    versionsIndex.collapse(
      presentedGames.filter((g) => settings.showHiddenGames || !g.hidden),
    ).length,
  );
  let favoriteCount = $derived(
    versionsIndex.collapse(
      presentedGames.filter(
        (g) => (settings.showHiddenGames || !g.hidden) && g.favorite,
      ),
    ).length,
  );
  let selected = $derived.by(() => {
    const game = games.find((g) => g.id === selectedId);
    return game
      ? settings.hideDuplicateGames
        ? versionsIndex.choose([
            versionsIndex.withPresentation(
              presentCached(game, providers.revision),
            ),
          ])
        : versionsIndex.withPresentation(
            presentCached(game, providers.revision),
          )
      : undefined;
  });
  let clockTime = $state(Date.now());
  let achievementProvider = $derived(
    selected && preferredCopy(selected)?.source === "emulated"
      ? ("retroachievements" as const)
      : ("achievements" as const),
  );
  let achievementRevision = $derived(
    achievementProvider === "retroachievements"
      ? providers.revision
      : (steamAchievementAccount?.revision ?? 0),
  );
  function linkRetroAchievements(
    gameId: string,
    copyId: string,
    id: number | undefined,
  ) {
    const game = gameIndex.get(gameId);
    if (!game) return;
    updateGame(gameId, {
      copies: game.copies.map((c) =>
        c.id === copyId && c.source === "emulated" && c.emulation
          ? { ...c, emulation: { ...c.emulation, retroAchievementsGameId: id } }
          : c,
      ),
    });
  }
  let resolvedCollections = $derived(
    resolveCollections(collections, presentedGames, clockTime),
  );
  let folderCounts = $derived(
    settings.hideDuplicateGames
      ? new Map(
          resolvedCollections.map((c) => [
            c.id,
            versionsIndex.collapse(
              [...new Set(c.gameIds)]
                .map((id) => gameIndex.get(id))
                .filter(
                  (g): g is Game =>
                    !!g && (!!settings.showHiddenGames || !g.hidden),
                ),
            ).length,
          ]),
        )
      : collectionCounts(resolvedCollections, browsableGames),
  );
  let totalFolderCounts = $derived(
    collectionCounts(resolvedCollections, games),
  );
  let matchingGames = $derived(
    filterGames(
      presentedGames,
      filter,
      query,
      installedOnly,
      tagFilter,
      sort,
      resolvedCollections,
      !!settings.showHiddenGames,
    ),
  );
  let filtered = $derived(
    settings.hideDuplicateGames
      ? filterGames(
          versionsIndex.collapse(matchingGames),
          "all",
          "",
          false,
          "",
          sort,
          [],
          true,
        )
      : matchingGames,
  );
  function browseUnavailable(game: Game) {
    const copy = preferredCopy(game);
    return !copy?.installed
      ? "Install this copy to browse its files."
      : copy.available === false
        ? "This installation is unavailable. Reconnect its drive and refresh installed games."
        : !copy.installPath && !copy.executable
          ? "No installation folder is known. Refresh installed games or add its executable."
          : demoMode || !api.desktop
            ? "Browse game files in the desktop app."
            : "";
  }
  function uninstallUnavailable(game: Game) {
    const copy = preferredCopy(game);
    return demoMode || !api.desktop
      ? "Uninstall from your desktop library."
      : !copy?.installed
        ? "This version is not installed."
        : copy.available === false
          ? "This installation is unavailable."
          : copy.emulation || copy.source === "emulated"
            ? "ROM files are managed with Browse Game Files; the shared emulator is kept."
            : "";
  }
  function showGameContext(
    event: MouseEvent,
    game: Game,
    slot?: "cover" | "hero",
  ) {
    if (slot && (event.target as Element).closest("button,a,select,input"))
      return;
    event.preventDefault();
    event.stopPropagation();
    const target = event.currentTarget as HTMLElement,
      rect = target.getBoundingClientRect();
    contextMenu = {
      gameId: game.id,
      copyId: preferredCopy(game)?.id,
      identity: gameContextIdentity(game, preferredCopy(game)),
      anchor: {
        x: event.clientX || rect.left + 12,
        y: event.clientY || rect.top + 12,
        target,
      },
      slot,
    };
  }
  function editGameDetails(game: Game) {
    if (selectedId !== game.id) openGame(game);
    metadataId = String(game.metadataId ?? "");
    openModal("metadata");
  }
  function editArtwork(game: Game, slot: "cover" | "hero" = "cover") {
    if (selectedId !== game.id) openGame(game);
    artworkSlot = slot;
    openModal("artwork");
  }
  const installWatch = new InstallationWatch();
  let checkingInstalls = $state(false);
  let lastInstallCheck = 0;
  async function refreshInstalls() {
    if (
      checkingInstalls ||
      busy ||
      demoMode ||
      !api.desktop ||
      !ready ||
      Date.now() - lastInstallCheck < 30_000
    )
      return;
    const requested = installWatch.pending(games);
    if (!requested.length) return;
    const epoch = libraryEpoch;
    const folders = JSON.stringify(settings.libraryFolders ?? {});
    checkingInstalls = true;
    lastInstallCheck = Date.now();
    try {
      const report = await api.scanInstalled(settings.libraryFolders ?? {}, [
        ...new Set(requested.map((job) => job.source)),
      ]);
      if (
        epoch !== libraryEpoch ||
        demoMode ||
        folders !== JSON.stringify(settings.libraryFolders ?? {})
      )
        return;
      const updated = installWatch.apply(games, report.games, requested);
      if (updated !== games) {
        games = updated;
        persist();
        notify("Installation detected. Your library is ready to play.");
      }
    } catch {
      /* Keep the last known state and retry on the next bounded check. */
    } finally {
      checkingInstalls = false;
    }
  }
  const pendingUninstalls = new Map<
    string,
    { gameId: string; copyId: string; startedAt: number }
  >();
  let checkingInstallations = false;
  async function refreshUninstalls(all = false) {
    if (checkingInstallations || demoMode || !api.desktop || !ready) return;
    for (const [key, job] of pendingUninstalls) {
      if (
        Date.now() - job.startedAt >= 30 * 60_000 ||
        !gameIndex
          .get(job.gameId)
          ?.copies.some((c) => c.id === job.copyId && c.installed)
      )
        pendingUninstalls.delete(key);
    }
    const copies = all
      ? games.flatMap((g) =>
          g.copies
            .filter((c) => c.installed && !c.emulation)
            .map((c) => ({ gameId: g.id, copyId: c.id })),
        )
      : [...pendingUninstalls.values()].filter(
          (v) => Date.now() - v.startedAt < 30 * 60_000,
        );
    if (!copies.length) return;
    const epoch = libraryEpoch;
    checkingInstallations = true;
    try {
      const statuses = await api.checkInstallations(copies);
      if (epoch !== libraryEpoch || demoMode) return;
      const missing = new Map(
        statuses.map((s) => [s.gameId + ":" + s.copy.id, s.copy]),
      );
      let changed = false;
      games = games.map((g) => {
        let updated = false;
        const copies = g.copies.map((c) => {
          const next = missing.get(g.id + ":" + c.id);
          if (
            next &&
            c.source === next.source &&
            c.sourceId === next.sourceId &&
            c.installPath === next.installPath &&
            c.executable === next.executable &&
            c.installed
          ) {
            updated = changed = true;
            pendingUninstalls.delete(g.id + ":" + c.id);
            return { ...c, installed: false, installSizeBytes: undefined };
          }
          return c;
        });
        return updated ? { ...g, copies } : g;
      });
      if (changed) persist();
    } catch {
      /* A failed source probe keeps the known installation evidence. */
    } finally {
      checkingInstallations = false;
    }
  }
  async function prepareUninstall(game: Game) {
    const copy = preferredCopy(game);
    if (!copy || uninstallUnavailable(game)) return;
    busy = `uninstall-plan:${game.id}`;
    try {
      await persistence;
      const plan = await api.planUninstall(game.id, copy.id);
      if (!gameIndex.get(game.id)?.copies.some((c) => c.id === copy.id)) return;
      uninstallRequest = { gameId: game.id, copyId: copy.id, plan };
      openModal("uninstall");
    } catch (e) {
      notify(String(e), true);
    } finally {
      busy = "";
    }
  }
  async function confirmUninstall() {
    const request = uninstallRequest;
    if (!request || busy) return;
    busy = `uninstall:${request.gameId}`;
    try {
      await persistence;
      const message = await api.uninstallGame(
        request.gameId,
        request.copyId,
        request.plan.token,
      );
      pendingUninstalls.set(request.gameId + ":" + request.copyId, {
        gameId: request.gameId,
        copyId: request.copyId,
        startedAt: Date.now(),
      });
      closeModal();
      notify(message);
    } catch (e) {
      formError = String(e);
    } finally {
      busy = "";
    }
  }
  function chooseVersion(copyId: string) {
    const version = selectedVersions.find((v) => v.copy.id === copyId);
    if (!version || !selectedId) return;
    const choice = { gameId: version.game.id, copyId, selectedAt: Date.now() };
    const preferences = { ...settings.versionPreferences };
    for (const member of versionsIndex.members(selectedId))
      preferences[member.id] = choice;
    settings.versionPreferences = preferences;
    games = games.map((g) =>
      g.id === version.game.id ? { ...g, preferredCopy: copyId } : g,
    );
    selectedId = version.game.id;
    window.history.replaceState({ slopGame: selectedId }, "");
    persist();
  }
  let groupingMode = $derived(
    groupOptions.some(([id]) => id === settings.libraryGrouping)
      ? settings.libraryGrouping!
      : ("none" as GroupMode),
  );
  let grouped = $derived(
    groupingMode === "none"
      ? []
      : groupGames(
          filtered,
          groupingMode,
          settings.smartGroups ?? [],
          clockTime,
        ),
  );
  let enrichmentPolicy = $derived(
    JSON.stringify([
      steamAchievementAccount?.revision,
      steamAchievementAccount?.connected,
      steamAchievementAccount?.reconnectRequired,
      providers.revision,
      providers.igdbConfigured,
      providers.steamGridDbConfigured,
      providers.screenScraperConfigured,
      providers.retroAchievementsConfigured,
      settings.autoFillGaps,
      [...ratingDetailIds],
      selectedId,
      modal,
      artworkRequests,
    ]),
  );
  function saveSmartGroups(rules: SmartGroup[]) {
    settings.smartGroups = validateGroups(rules);
    settings.libraryGrouping = rules.length ? "smart" : "none";
    persistSettings();
    closeModal();
  }
  let tags = $derived(
    libraryTags(
      settings.showHiddenGames
        ? presentedGames
        : presentedGames.filter((g) => !g.hidden),
    ),
  );
  let gapCount = $derived(
    presentedGames.filter((g) => missingFields(g).length > 0).length,
  );
  let artworkCredits = $derived(
    selected
      ? (["cover", "hero"] as const).flatMap((slot) => {
          const asset = selectedArtwork(selected!, slot);
          return asset?.page ? [{ ...asset, slot }] : [];
        })
      : [],
  );
  let title = $derived(
    isCollectionFilter(filter)
      ? (activeCollection?.name ?? "Collection")
      : filter === "all"
        ? "Library"
        : filter === "recent"
          ? "Recently played"
          : filter === "favorites"
            ? "Favorites"
            : filter === "hidden"
              ? "Hidden Games"
              : sourceNames[filter],
  );
  let installedCount = $derived(
    versionsIndex.collapse(
      presentedGames.filter(
        (g) =>
          (settings.showHiddenGames || !g.hidden) &&
          g.copies.some((c) => c.installed),
      ),
    ).length,
  );

  async function openResource(url: string) {
    try {
      await api.openResource(url);
    } catch (e) {
      notify(`Could not open link: ${e}`, true);
    }
  }
  function notify(message: string, error = false) {
    toast = message;
    toastError = error;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ""), 6500);
  }
  const failedWrites = new Set<string>();
  function saved(scopes: string[] = [], full = false) {
    if (full) failedWrites.clear();
    else for (const scope of scopes) failedWrites.delete(scope);
    saveStatus = failedWrites.size ? "error" : "saved";
  }
  function failed(scopes: string[]) {
    for (const scope of scopes) failedWrites.add(scope);
    saveStatus = "error";
  }
  // All writes join `persistence`: metadata, flags and full snapshots must
  // reach SQLite in UI order so an older snapshot cannot undo a newer edit.
  function flushEnrichment() {
    clearTimeout(enrichmentSaveTimer);
    enrichmentSaveTimer = undefined;
    lastEnrichmentSave = Date.now();
    if (demoMode || !ready) {
      dirtyEnrichment.clear();
      return;
    }
    const ids = [...dirtyEnrichment];
    dirtyEnrichment.clear();
    if (!ids.length) return;
    const updates = $state.snapshot(
      ids.map((id) => gameIndex.get(id)).filter((g): g is Game => !!g),
    );
    saveStatus = "saving";
    persistence = persistence
      .catch(() => {})
      .then(() => api.saveEnrichment(updates))
      .then(() => {
        saved(ids.map((id) => `metadata:${id}`));
      })
      .catch((e) => {
        for (const id of ids) dirtyEnrichment.add(id);
        failed(ids.map((id) => `metadata:${id}`));
        notify(`Could not save metadata: ${e}`, true);
      });
  }
  function persist() {
    clearTimeout(notesSaveTimer);
    notesSaveTimer = undefined;
    if (demoMode || !ready) return;
    dirtyEnrichment.clear();
    clearTimeout(enrichmentSaveTimer);
    enrichmentSaveTimer = undefined;
    saveStatus = "saving";
    // Svelte's snapshot already detaches reactive state. A second deep clone
    // doubles allocation for every edit/import in a large library.
    const snapshot = $state.snapshot({ games, settings });
    persistence = persistence
      .catch(() => {})
      .then(() => api.saveLibrary(snapshot))
      .then(() => {
        saved([], true);
      })
      .catch((e) => {
        failed(["library"]);
        notify(`Could not save your library: ${e}`, true);
      });
  }
  function persistSettings() {
    if (demoMode || !ready) return;
    saveStatus = "saving";
    const snapshot = $state.snapshot(settings);
    persistence = persistence
      .catch(() => {})
      .then(() => api.saveSettings(snapshot))
      .then(() => {
        saved(["settings", "theme"]);
      })
      .catch((e) => {
        failed(["settings"]);
        notify(`Could not save settings: ${e}`, true);
      });
  }
  function updateGame(id: string, changes: Partial<Game>) {
    games = games.map((g) => (g.id === id ? { ...g, ...changes } : g));
    persist();
  }
  function updateNotes(id: string, value: string) {
    games = games.map((g) => (g.id === id ? { ...g, notes: value } : g));
    if (demoMode || !ready) return;
    saveStatus = "saving";
    clearTimeout(notesSaveTimer);
    notesSaveTimer = setTimeout(persist, 250);
  }
  function setTheme(theme: ThemeId) {
    settings.theme = theme;
    saveStatus = "saving";
    // Appearance is global, including while viewing the separate sample collection.
    persistence = persistence
      .catch(() => {})
      .then(() => api.saveTheme(theme))
      .then(() => {
        saved(["theme"]);
      })
      .catch((error) => {
        failed(["theme"]);
        notify(`Could not save your theme: ${error}`, true);
      });
  }
  function setProviderId(
    provider:
      | "igdb"
      | "steamgriddb"
      | "hltb"
      | "screenscraper"
      | "screenScraperSystem"
      | "gogReviews",
    text: string,
  ) {
    if (!selected) return;
    const id = text.trim() ? Number(text) : undefined;
    if (id !== undefined && (!Number.isSafeInteger(id) || id <= 0)) {
      notify("Enter a positive numeric game ID, or leave it blank.", true);
      return;
    }
    updateGame(selected.id, {
      providerIds: { ...selected.providerIds, [provider]: id },
    });
  }
  function removeManualGame() {
    if (
      !selected ||
      !selected.copies.every(
        (c) => c.source === "manual" || c.source === "emulated",
      )
    )
      return;
    const title = selected.title;
    if (settings.collections)
      settings.collections = withoutGame(settings.collections, selected.id);
    games = games.filter((g) => g.id !== selectedId);
    selectedId = null;
    window.history.replaceState({}, "");
    persist();
    closeModal();
    notify(`${title} removed from the library. Game files were kept.`);
  }
  function favorite(g: Game) {
    const result = changeDuplicateFlag(
      games,
      versionsIndex,
      g.id,
      "favorite",
      !g.favorite,
    );
    games = result.games;
    persistFlags(result.updates);
  }
  function hideGame(g: Game) {
    const result = changeDuplicateFlag(
      games,
      versionsIndex,
      g.id,
      "hidden",
      !g.hidden,
    );
    games = result.games;
    persistFlags(result.updates);
  }
  function persistFlags(updates: api.GameFlagsUpdate[]) {
    if (demoMode || !ready) return;
    saveStatus = "saving";
    // Keep ordering with imports/full saves, without cloning metadata or touching
    // pending notes/enrichment. Only these personal flags cross the IPC boundary.
    persistence = persistence
      .catch(() => {})
      .then(() => api.saveGameFlags(updates))
      .then(() => {
        saved(
          updates.flatMap((u) =>
            Object.keys(u)
              .filter((key) => key !== "id")
              .map((key) => `flag:${u.id}:${key}`),
          ),
        );
      })
      .catch((e) => {
        failed(
          updates.flatMap((u) =>
            Object.keys(u)
              .filter((key) => key !== "id")
              .map((key) => `flag:${u.id}:${key}`),
          ),
        );
        notify(`Could not save your game preference: ${e}`, true);
      });
  }
  function collectionFromGroup(
    id: string,
    rules = settings.smartGroups ?? [],
    mode = groupingMode,
    snapshotIds?: string[],
  ) {
    if (demoMode || mode === "none") return;
    const groups =
      mode === "smart" ? validateGroups($state.snapshot(rules)) : [];
    const match = groupGames(presentedGames, mode, groups, clockTime).find(
      (g) => g.id === id,
    );
    const rule = { mode, groupId: id, groups };
    collectionDraft = {
      id: crypto.randomUUID(),
      name:
        match?.name ??
        groups.find((g) => `smart:${g.id}` === id)?.name ??
        "New collection",
      gameIds: snapshotIds ?? match?.games.map((g) => g.id) ?? [],
      rule,
    };
    openModal("collection");
  }
  function editCollection(collection?: GameCollection, gameId?: string) {
    if (demoMode) return;
    collectionDraft = collection
      ? structuredClone($state.snapshot(collection))
      : { id: crypto.randomUUID(), name: "", gameIds: gameId ? [gameId] : [] };
    openModal("collection");
  }
  function commitCollection(value: GameCollection) {
    if (demoMode) return;
    const existing = collections.some((c) => c.id === value.id);
    settings.collections = saveCollection(collections, value, games);
    persistSettings();
    collectionReturnToGroups = false;
    groupingEditorDraft = null;
    closeModal();
    if (!existing && !selectedId) navigate(`collection:${value.id}`);
    notify(existing ? "Collection saved." : "Collection created.");
  }
  function deleteCollection() {
    const id = collectionDraft.id;
    settings.collections = collections.filter((c) => c.id !== id);
    persistSettings();
    closeModal();
    if (filter === `collection:${id}`) navigate("all");
    notify("Collection deleted. Your games were kept.");
  }
  function changeMembership(collectionId: string, included: boolean) {
    if (demoMode || !selectedId || !games.some((g) => g.id === selectedId))
      return;
    settings.collections = setMembership(
      collections,
      collectionId,
      selectedId,
      included,
    );
    persistSettings();
  }
  function setNexusDomain(text: string) {
    if (!selected) return;
    const domain = text.trim().toLowerCase() || undefined;
    if (domain && !/^[a-z0-9][a-z0-9-]{0,99}$/.test(domain)) {
      notify(
        "Enter the game name from its Nexus URL, or leave it blank.",
        true,
      );
      return;
    }
    updateGame(selected.id, {
      providerIds: { ...selected.providerIds, nexus: domain },
    });
  }
  function navigate(value: Filter) {
    page = "library";
    selectedId = null;
    filter = value;
    if (value === "hidden") installedOnly = false;
    query = "";
    tagFilter = "";
    window.history.replaceState({}, "");
  }
  function closeDetails() {
    if (window.history.state?.slopGame) window.history.back();
    else selectedId = null;
  }
  function showSetup() {
    closeModal();
    selectedId = null;
    page = "setup";
    window.history.replaceState({}, "");
  }
  function showSettings() {
    closeModal();
    selectedId = null;
    page = "settings";
    window.history.replaceState({}, "");
  }
  function showSources() {
    page = "sources";
    selectedId = null;
    window.history.replaceState({}, "");
  }
  function popstate() {
    const id = window.history.state?.slopGame;
    selectedId = games.some((g) => g.id === id) ? id : null;
    if (selectedId) page = "library";
  }
  function openModal(value: typeof modal) {
    if (value === "grouping") groupingEditorDraft = null;
    formError = "";
    if (value === "metadata") {
      metadataId = String(selected?.metadataId ?? "");
      metadataQuery = selected?.title ?? "";
      metadataResults = [];
      metadataSearched = false;
    }
    modal = value;
    if (value === "artwork" && artworkGame)
      artworkRequests = [artworkIdentity(artworkGame)];
  }
  function chooseArtwork(
    game: Game,
    slot: "cover" | "hero",
    asset: (ArtworkAsset & { source: ArtworkChoice["source"] }) | null,
  ) {
    const current = games.find((g) => g.id === game.id);
    if (!current || artworkIdentity(current) !== artworkIdentity(game)) return;
    const artwork = { ...current.artwork };
    if (asset)
      artwork[slot] = {
        ...asset,
        identity: artworkIdentity(current),
        selectedAt: Date.now(),
      };
    else delete artwork[slot];
    updateGame(current.id, { artwork });
  }
  async function searchMetadata() {
    if (busy || !api.desktop || demoMode || !selected || !metadataQuery.trim())
      return;
    const request = ++metadataRequest,
      gameId = selected.id,
      epoch = libraryEpoch,
      query = metadataQuery;
    busy = "search-metadata";
    formError = "";
    try {
      const results = await api.searchMetadata(query);
      if (
        request !== metadataRequest ||
        epoch !== libraryEpoch ||
        selectedId !== gameId ||
        modal !== "metadata" ||
        query !== metadataQuery
      )
        return;
      metadataResults = results;
      metadataSearched = true;
    } catch (e) {
      if (request === metadataRequest && modal === "metadata")
        formError = String(e);
    } finally {
      if (request === metadataRequest) busy = "";
    }
  }
  async function applyMatch(id: number) {
    if (!selected || busy) return;
    const request = ++metadataRequest,
      game = selected;
    busy = "metadata";
    formError = "";
    try {
      await enrich(game, id);
      if (
        request === metadataRequest &&
        modal === "metadata" &&
        selectedId === game.id
      )
        closeModal();
    } catch (e) {
      if (request === metadataRequest && modal === "metadata")
        formError = String(e);
    } finally {
      if (request === metadataRequest) busy = "";
    }
  }
  function closeModal() {
    if (modal === "collection" && collectionReturnToGroups) {
      collectionReturnToGroups = false;
      modal = "grouping";
      return;
    }
    if (modal === "grouping") groupingEditorDraft = null;
    if (modal === "metadata") {
      metadataRequest++;
      if (busy === "search-metadata" || busy === "metadata") busy = "";
    }
    modal = null;
  }
  $effect(() => {
    if (!ready || demoMode) return;
    const titles = reconcileStoreTitles(games, providers.revision);
    if (titles !== games) {
      untrack(() => {
        games = titles;
        persist();
      });
      return;
    }
    const result = reconcileDuplicateFlags(games, versionsIndex);
    if (result.updates.length)
      untrack(() => {
        games = result.games;
        persistFlags(result.updates);
      });
  });
  $effect(() => {
    if (
      modal === "artwork" &&
      (!artworkGame || !artworkRequests.includes(artworkIdentity(artworkGame)))
    )
      closeModal();
  });
  $effect(() => {
    const state = contextMenu;
    if (!state) return;
    if (!contextGame) {
      contextMenu = null;
      return;
    }
    void filtered;
    void page;
    void selectedId;
    const target = state.anchor.target;
    void tick().then(() => {
      if (contextMenu === state && target && !target.isConnected)
        contextMenu = null;
    });
  });
  $effect(() => {
    if (modal && dialog && !dialog.open) dialog.showModal();
    if (!modal && dialog?.open) dialog.close();
  });
  $effect(() => {
    applyTheme(document.documentElement, settings.theme);
  });
  $effect(() => {
    void filter;
    void query;
    void installedOnly;
    void tagFilter;
    void sort;
    void settings.libraryGrouping;
    void settings.smartGroups;
    libraryScroll = 0;
  });
  $effect(() => {
    void enrichmentPolicy;
    if (api.desktop && ready && !demoMode)
      enrichmentQueue.scheduleReconcile(games, providers.revision, selectedId);
    else enrichmentQueue.clear();
  });
  function reconcileEnrichment() {
    if (api.desktop && ready && !demoMode)
      enrichmentQueue.scheduleReconcile(games, providers.revision, selectedId);
  }
  function keyboard(e: KeyboardEvent) {
    if (e.defaultPrevented || document.querySelector("dialog[open]")) return;
    const editing =
      e.target instanceof Element &&
      !!e.target.closest("input, textarea, select, [contenteditable='true']");
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      if (modal) return;
      page = "library";
      selectedId = null;
      window.history.replaceState({}, "");
      tick().then(() => searchInput?.focus());
    }
    if (
      e.key === "Escape" &&
      !editing &&
      selectedId &&
      !modal &&
      !document.querySelector("[data-game-menu]:popover-open")
    )
      closeDetails();
  }

  async function browseGameFiles(game: Game) {
    const copy = preferredCopy(game);
    if (!copy) return;
    try {
      await persistence;
      if (saveStatus === "error")
        throw new Error("Save your library before browsing game files.");
      await api.browseGameFiles(game.id, copy.id);
    } catch (error) {
      notify(String(error), true);
    }
  }

  async function scan() {
    if (busy || !ready) return;
    const epoch = libraryEpoch;
    if (demoMode) {
      notify("Leave the sample library to scan your installed games.");
      return;
    }
    busy = "scan";
    try {
      const folders = JSON.stringify(settings.libraryFolders ?? {});
      const before = new Set(games.flatMap((g) => g.copies.map((c) => c.id)));
      const report = await api.scanInstalled(settings.libraryFolders ?? {});
      if (
        epoch !== libraryEpoch ||
        demoMode ||
        folders !== JSON.stringify(settings.libraryFolders ?? {})
      )
        return;
      const current = new Set(games.flatMap((g) => g.copies.map((c) => c.id)));
      games = mergeDiscovered(
        games,
        report.games.filter((g) =>
          g.copies.every((c) => !before.has(c.id) || current.has(c.id)),
        ),
      );
      scanMessages = report.messages;
      persist();
      notify(`Found ${report.games.length} installed game records.`);
      void refreshUninstalls(true);
    } catch (e) {
      notify(String(e), true);
    } finally {
      busy = "";
    }
  }
  async function changeFolders(source: AccountSource, paths: string[]) {
    if (!ready || demoMode)
      throw new Error("Open your own library before changing source folders.");
    const previous = settings.libraryFolders?.[source] ?? [];
    settings = {
      ...settings,
      libraryFolders: { ...settings.libraryFolders, [source]: paths },
    };
    persistSettings();
    await persistence;
    if (saveStatus === "error")
      throw new Error("The folder list could not be saved. Try again.");
    if (paths.length > previous.length) await scan();
    else
      notify("Folder removed from your list. Your games and files were kept.");
  }
  function openGame(g: Game) {
    window.history.pushState({ slopGame: g.id }, "");
    selectedId = g.id;
  }
  async function enrich(g: Game, id: number, showErrors = true) {
    const initialIdentity = identity(g, "pcgw");
    const epoch = libraryEpoch;
    try {
      const m = await api.steamMetadata(id);
      const latest = games.find((current) => current.id === g.id);
      if (
        epoch !== libraryEpoch ||
        demoMode ||
        !latest ||
        identity(latest, "pcgw") !== initialIdentity
      )
        return;
      const patch: Partial<Game> = {
        metadataId: id,
        cover:
          latest.copies.some((c) => c.source === "steam") || !latest.cover
            ? m.cover
            : latest.cover,
        hero:
          latest.copies.some((c) => c.source === "steam") || !latest.hero
            ? m.hero
            : latest.hero,
        description: m.description,
        developer: m.developer,
        publisher: m.publisher,
        genres: m.genres,
        year: m.year,
        screenshots: m.screenshots,
      };
      if (!latest.metadataLocked) updateGame(g.id, patch);
      if (showErrors) notify("Game details updated.");
    } catch (e) {
      if (showErrors) throw e;
    }
  }
  async function launch(g: Game) {
    if (busy) return;
    if (demoMode) {
      notify("Sample game — switch to your library to launch installed games.");
      return;
    }
    const action = launchAction(g);
    if (action.unavailable) {
      notify(action.unavailable, true);
      return;
    }
    if (action.storeUrl) {
      await openResource(action.storeUrl);
      return;
    }
    busy = `launch:${g.id}`;
    const epoch = libraryEpoch;
    try {
      await persistence;
      if (saveStatus === "error")
        throw new Error(
          "Save your library successfully before launching a game.",
        );
      const copy = preferredCopy(g);
      if (!copy) throw new Error("No launch copy is configured.");
      const message = await api.launchGame(g.id, copy.id);
      if (epoch !== libraryEpoch || demoMode) return;
      installWatch.start(g, copy);
      notify(message); /* A launch request does not prove a play session. */
    } catch (e) {
      notify(String(e), true);
    } finally {
      busy = "";
    }
  }
  async function browse() {
    const epoch = libraryEpoch;
    try {
      const path = await api.chooseExecutable();
      if (epoch !== libraryEpoch || modal !== "add" || addMode !== "pc") return;
      if (path) {
        addPath = path;
        if (!addTitle)
          addTitle =
            path
              .split(/[\\/]/)
              .at(-1)
              ?.replace(/\.exe$/i, "") ?? "";
      } else if (!api.desktop)
        formError = "Choose an executable from the desktop app.";
    } catch (e) {
      formError = String(e);
    }
  }
  async function addGame(e: SubmitEvent) {
    e.preventDefault();
    if (busy || !ready) return;
    const epoch = libraryEpoch;
    busy = "add";
    formError = "";
    try {
      if (demoMode)
        throw new Error("Switch to your own library before adding a game.");
      const id = addSteamId.trim() ? Number(addSteamId) : undefined;
      if (id !== undefined && (!Number.isInteger(id) || id <= 0))
        throw new Error("Enter a numeric Steam app ID, or leave it blank.");
      const game = await api.manualGame(addTitle, addPath, id);
      if (epoch !== libraryEpoch || demoMode) return;
      games = [...games, game];
      persist();
      if (modal === "add") {
        closeModal();
        selectedId = game.id;
      }
      if (id) await enrich(game, id, false);
      notify(`${game.title} added to your library.`);
      addTitle = "";
      addPath = "";
      addSteamId = "";
    } catch (e) {
      formError = String(e);
    } finally {
      busy = "";
    }
  }
  async function saveEmulatorProfiles(profiles: EmulatorProfile[]) {
    if (demoMode)
      throw new Error(
        "Return to your library before changing emulator profiles.",
      );
    settings = { ...settings, emulators: profiles };
    persistSettings();
    await persistence;
    if (saveStatus === "error")
      throw new Error("Emulator profiles could not be saved. Try again.");
  }
  async function addEmulatedGame(input: EmulatedGameInput) {
    const epoch = libraryEpoch;
    if (demoMode)
      throw new Error("Return to your library before adding emulated games.");
    await persistence;
    if (saveStatus === "error")
      throw new Error("Save your library before adding another game.");
    const game = await api.addEmulated(input);
    if (epoch !== libraryEpoch || demoMode) return;
    games = [...games, game];
    persist();
    if (modal === "add") {
      closeModal();
      selectedId = game.id;
    }
    notify(`${game.title} added to your library.`);
  }
  async function saveEmulatedGame(input: EmulatedGameInput) {
    const epoch = libraryEpoch;
    if (!selected) return;
    const gameId = selected.id;
    const copy = preferredCopy(selected);
    if (!copy?.emulation) return;
    await persistence;
    const updated = await api.updateEmulated(
      gameId,
      copy.id,
      input.profileId,
      input.gamePath,
    );
    const current = games.find((g) => g.id === gameId);
    if (epoch !== libraryEpoch || demoMode || !current) return;
    updateGame(gameId, {
      title: input.title,
      copies: current.copies.map((c) => (c.id === copy.id ? updated : c)),
    });
    if (modal === "emulation" && selectedId === gameId) closeModal();
    notify("Emulator launch settings saved.");
  }
  async function matchMetadata(e: SubmitEvent) {
    e.preventDefault();
    const id = Number(metadataId);
    if (!Number.isSafeInteger(id) || id <= 0) {
      formError = "Enter a valid Steam app ID.";
      return;
    }
    await applyMatch(id);
  }
  function toggleDemo() {
    if (!ready) return;
    if (notesSaveTimer) persist();
    libraryEpoch++;
    installWatch.clear();
    pendingUninstalls.clear();
    closeModal();
    selectedId = null;
    query = "";
    filter = "all";
    page = "library";
    window.history.replaceState({}, "");
    if (demoMode) {
      demoMode = false;
      void loadLibrary();
    } else {
      demoMode = true;
      games = demoGames();
    }
    selectedId = null;
    query = "";
    filter = "all";
    page = "library";
  }
  async function loadLibrary() {
    const epoch = ++libraryEpoch;
    installWatch.clear();
    pendingUninstalls.clear();
    ready = false;
    loadError = "";
    try {
      await persistence;
      const library = await api.loadLibrary();
      if (epoch !== libraryEpoch) return;
      settings = {
        ...defaultSettings,
        ...library.settings,
        theme: resolveTheme(library.settings.theme).id,
      };
      installedOnly = settings.startInstalled;
      sort = sortOptions.some(([id]) => id === settings.librarySort)
        ? settings.librarySort!
        : "title";
      games = library.games;
      if (!api.desktop && !games.length) {
        demoMode = true;
        games = demoGames();
      }
      ready = true;
      if (api.desktop) {
        void accountManager
          .load()
          .then(async () => {
            const repair = needsAmazonPortraitRefresh(games)
              ? accountManager.sync("amazon", true)
              : Promise.resolve();
            await Promise.allSettled([repair, accountManager.syncAll(true)]);
          })
          .catch((e) => notify(`Connections could not load: ${e}`, true));
        if (!games.length) void scan();
      }
    } catch (e) {
      if (epoch !== libraryEpoch) return;
      loadError = String(e);
      notify(`Library could not load: ${e}`, true);
    }
  }
  onMount(() => {
    if (api.desktop)
      api
        .providerStatus()
        .then((status) => (providers = status))
        .catch((e) => notify(String(e), true));
    const retryTimer = setInterval(() => {
      clockTime = Date.now();
      void refreshUninstalls();
      void refreshInstalls();
      reconcileEnrichment();
      void accountManager.syncAll(true);
    }, 60_000);
    let disposed = false;
    let stopArtwork: (() => void) | undefined;
    void api
      .listenArtworkDownloads((delta) => {
        artworkDownloads = Math.max(0, artworkDownloads + delta);
      })
      .then((stop) => {
        if (disposed) stop();
        else stopArtwork = stop;
      })
      .catch(() => {})
      .finally(() => {
        if (!disposed) void loadLibrary();
      });
    return () => {
      disposed = true;
      stopArtwork?.();
      api.setLibraryArtwork([]);
      clearTimeout(toastTimer);
      clearTimeout(enrichmentSaveTimer);
      clearTimeout(notesSaveTimer);
      clearInterval(retryTimer);
      enrichmentQueue.clear();
      accountManager.dispose();
    };
  });
</script>

<svelte:window
  onkeydown={keyboard}
  onpopstate={popstate}
  onfocus={() => {
    clockTime = Date.now();
    void refreshUninstalls();
    void refreshInstalls();
    reconcileEnrichment();
    void accountManager.syncAll(true);
  }}
  ononline={() => {
    reconcileEnrichment();
    void accountManager.syncAll(true);
  }}
/>
<div class="app-shell">
  <aside class="sidebar">
    <button
      class="brand"
      onclick={() => navigate("all")}
      aria-label="SlopLauncher library"
      ><span class="brand-mark"><AppLogo /></span><span
        >slop<span class="brand-light">launcher</span></span
      ></button
    >
    <nav class="primary-nav" aria-label="Main navigation">
      <button
        class:nav-active={page === "library" && filter === "all"}
        onclick={() => navigate("all")}
        ><Library size={19} /><span>Library</span><span class="nav-count"
          >{libraryCount}</span
        ></button
      >
      <button
        class:nav-active={page === "library" && filter === "recent"}
        onclick={() => {
          navigate("recent");
          sort = "recent";
        }}><Clock3 size={18} /><span>Recently played</span></button
      >
      <button
        class:nav-active={page === "library" && filter === "favorites"}
        onclick={() => navigate("favorites")}
        ><Heart size={18} /><span>Favorites</span><span class="nav-count"
          >{favoriteCount || ""}</span
        ></button
      >
      <button
        class:nav-active={page === "library" && filter === "hidden"}
        aria-label="Hidden Games"
        title="Hidden Games"
        onclick={() => navigate("hidden")}
        ><EyeOff size={18} /><span>Hidden Games</span><span
          class="nav-count"
          aria-hidden="true">{hiddenCount}</span
        ></button
      >
    </nav>
    <div class="sidebar-library-nav">
      <div class="nav-heading">
        COLLECTIONS <button
          aria-label="New collection"
          title={demoMode
            ? "Use your own library to create collections"
            : "Create a collection"}
          disabled={demoMode || !ready}
          onclick={() => editCollection()}><Plus size={14} /></button
        >
      </div>
      <nav class="collection-nav" aria-label="Collections">
        {#each collections as collection (collection.id)}<button
            class:nav-active={page === "library" &&
              filter === `collection:${collection.id}`}
            onclick={() => navigate(`collection:${collection.id}`)}
            title={collection.name}
            ><Folder size={17} /><span class="collection-nav-name"
              >{collection.name}</span
            ><span class="nav-count"
              >{folderCounts.get(collection.id) ?? 0}</span
            ></button
          >
        {:else}<span class="collection-nav-empty"
            >{demoMode
              ? "Your library, your collections"
              : "No collections yet"}</span
          >{/each}
      </nav>
      <div class="nav-heading">
        YOUR SOURCES <button aria-label="Manage sources" onclick={showSources}
          ><Plus size={14} /></button
        >
      </div>
      <nav class="source-nav" aria-label="Game sources">
        {#each sources as source}<button
            class:nav-active={page === "library" && filter === source}
            onclick={() => navigate(source)}
            ><span class="store-icon" data-source={source}
              >{source === "steam"
                ? "S"
                : source === "epic"
                  ? "E"
                  : source === "amazon"
                    ? "a"
                    : source === "gog"
                      ? "gog"
                      : source === "itch"
                        ? "i"
                        : source === "emulated"
                          ? "◈"
                          : "+"}</span
            ><span>{sourceNames[source]}</span><span class="nav-count"
              >{browsableGames.filter((g) =>
                g.copies.some((c) => c.source === source),
              ).length || ""}</span
            ></button
          >{/each}
      </nav>
    </div>
    <div class="sidebar-bottom">
      {#if demoMode}<button class="sample-indicator" onclick={toggleDemo}
          ><FlaskConical size={15} /><span>Sample library</span><ArrowUpRight
            size={14}
          /></button
        >{/if}
      <button class:nav-active={page === "sources"} onclick={showSources}
        ><Link2 size={17} />Sources</button
      >
      <button
        class:nav-active={page === "settings"}
        disabled={!ready}
        onclick={showSettings}><Settings2 size={17} />Settings</button
      >
      <div class="sidebar-footer">
        <span class="status-dot"></span>{api.desktop
          ? "Local library"
          : "Browser preview"}<span>v{appVersion}</span>
      </div>
    </div>
  </aside>

  <div class="workspace">
    <header class="topbar">
      <div class="breadcrumbs">
        {#if selected}
          <button
            class="library-back-button"
            onclick={closeDetails}
            title="Return to your library"
            ><ArrowLeft size={15} />Library</button
          >
        {:else}<span>Your space</span>{/if}<ChevronRight size={13} /><span
          >{selected
            ? selected.title
            : page === "setup"
              ? "Setup"
              : page === "settings"
                ? "Settings"
                : page === "sources"
                  ? "Sources"
                  : title}</span
        >
      </div>
      <div class="topbar-actions">
        <button
          class="icon-button"
          aria-label="Refresh installed games"
          disabled={!!busy || demoMode || !api.desktop || !ready}
          onclick={scan}
          ><RefreshCw size={17} class={busy === "scan" ? "spin" : ""} /></button
        >
        <div class="topbar-divider"></div>
        <button
          class="add-game-button"
          disabled={!ready || demoMode}
          onclick={() => openModal("add")}><Plus size={16} />Add game</button
        >
      </div>
    </header>

    {#if page === "setup"}
      <SetupWizard
        {games}
        {settings}
        {providers}
        accounts={accountStates}
        manager={accountManager}
        disabled={demoMode || !ready}
        onproviders={(status) => (providers = status)}
        onfolders={changeFolders}
        onemulators={saveEmulatorProfiles}
        onclose={showSources}
      />
    {:else if page === "sources"}
      <main class="sources-page">
        <div class="page-eyebrow">CONNECTIONS</div>
        <h1>Sources</h1>
        <button class="secondary-button" onclick={showSetup}
          >Setup wizard</button
        >
        <p class="page-description">
          Connect accounts for your full owned libraries. Add folders below to
          find installed copies across drives. Manual and emulated games use Add
          game.
        </p>
        {#if demoMode}<div class="inline-notice">
            <FlaskConical size={17} /><span>You’re viewing sample games.</span
            ><button onclick={toggleDemo}
              >Use my library <ArrowUpRight size={14} /></button
            >
          </div>{/if}
        <SourceConnections
          {games}
          accounts={accountStates}
          manager={accountManager}
          disabled={demoMode || !!busy || !ready}
          steamId={settings.steamId}
          folders={settings.libraryFolders ?? {}}
          onFoldersChange={changeFolders}
        />
        <section class="discovery-panel">
          <HardDrive size={22} />
          <div>
            <h2>Already installed?</h2>
            <p>
              Read local Steam, Epic, Amazon, GOG, and itch.io records without
              changing your game files.
            </p>
          </div>
          <button
            class="secondary-button"
            disabled={!!busy || demoMode || !api.desktop || !ready}
            onclick={scan}
            >{#if busy === "scan"}<LoaderCircle
                size={15}
                class="spin"
              />{:else}<RefreshCw size={15} />{/if}Scan installed games</button
          >
        </section>
        {#if scanMessages.length}<details class="scan-details">
            <summary>Last import details</summary
            >{#each scanMessages as message}<p>{message}</p>{/each}
          </details>{/if}
      </main>
    {:else if page === "settings"}
      <main class="settings-page" aria-labelledby="settings-page-title">
        <div class="settings-page-heading">
          <div>
            <div class="page-eyebrow">PREFERENCES</div>
            <h1 id="settings-page-title">Settings</h1>
          </div>
          <span class="settings-save-state"
            >{saveStatus === "error"
              ? "Could not save changes"
              : "Changes save automatically"}{#if saveStatus === "error"}<button
                class="text-button"
                onclick={() =>
                  demoMode ? setTheme(settings.theme) : persist()}
                >Retry save</button
              >{/if}</span
          >
        </div>
        <div class="setup-entry">
          <p>
            Need help connecting your games, artwork or achievements? Set up
            only the services you want.
          </p>
          <button class="secondary-button" onclick={showSetup}
            >Setup wizard</button
          >
        </div>
        <div class="settings-columns">
          <div class="settings-column">
            <section
              class="settings-panel"
              aria-label="Appearance and library preferences"
            >
              <h2>Appearance & library</h2>
              <ThemePicker value={settings.theme} onchange={setTheme} />
              <div class="setting-row">
                <div>
                  <strong>Compact library</strong>
                  <p>Smaller covers and tighter spacing to show more rows.</p>
                </div>
                <input
                  aria-label="Compact library"
                  type="checkbox"
                  checked={!settings.comfortableLibrary}
                  onchange={(event) => {
                    settings.comfortableLibrary = !event.currentTarget.checked;
                    persistSettings();
                  }}
                />
              </div>
              <div class="setting-row">
                <div>
                  <strong>Cover size</strong>
                  <p>
                    {Math.round(
                      settings.coverSize *
                        (settings.comfortableLibrary ? 1 : 0.75),
                    )}px minimum width
                  </p>
                </div>
                <input
                  aria-label="Settings cover size"
                  type="range"
                  min="140"
                  max="250"
                  step="10"
                  bind:value={settings.coverSize}
                  onchange={persistSettings}
                />
              </div>
              <div class="setting-row">
                <div>
                  <strong>Automatic library sync</strong>
                  <p>
                    Check connected stores for newly owned games. The schedule
                    survives restarts. Failed syncs wait at least one hour; Sync
                    in Sources is always available.
                  </p>
                </div>
                <select
                  aria-label="Automatic library sync"
                  value={settings.librarySyncHours ?? 6}
                  onchange={(event) => {
                    settings.librarySyncHours = Number(
                      event.currentTarget.value,
                    );
                    persistSettings();
                  }}
                >
                  <option value={0}>Manual only</option>
                  <option value={1}>Every hour</option>
                  <option value={6}>Every 6 hours</option>
                  <option value={12}>Every 12 hours</option>
                  <option value={24}>Daily</option>
                </select>
              </div>
              <div class="setting-row">
                <div>
                  <strong>Automatically fill missing artwork & metadata</strong>
                  <p>
                    Check at startup and for new or changed games. Completed
                    games are skipped; your artwork choices stay in place.
                  </p>
                </div>
                <input
                  aria-label="Automatically fill missing artwork and metadata"
                  type="checkbox"
                  checked={settings.autoFillGaps !== false}
                  onchange={(event) => {
                    settings.autoFillGaps = event.currentTarget.checked;
                    persistSettings();
                    reconcileEnrichment();
                  }}
                />
              </div>

              <div class="setting-row">
                <div>
                  <strong>Start with installed games</strong>
                  <p>Show games ready to play when opening the app.</p>
                </div>
                <input
                  aria-label="Start with installed games"
                  type="checkbox"
                  bind:checked={settings.startInstalled}
                  onchange={persistSettings}
                />
              </div>
              <div class="setting-row">
                <div>
                  <strong>Hide duplicate games</strong>
                  <p>
                    Show one version of matching games. All copies and personal
                    data are kept; switch versions in game details. Favorites
                    and hidden status always apply to all matching copies, even
                    when this option is off.
                  </p>
                </div>
                <input
                  type="checkbox"
                  aria-label="Hide duplicate games"
                  checked={!!settings.hideDuplicateGames}
                  onchange={(e) => {
                    settings.hideDuplicateGames = e.currentTarget.checked;
                    persistSettings();
                  }}
                />
              </div>
              <div class="setting-row">
                <div>
                  <strong>Preferred platform</strong>
                  <p>
                    Default version for duplicates. A manually selected version
                    takes priority.
                  </p>
                </div>
                <select
                  aria-label="Preferred platform"
                  value={settings.preferredSource ?? "steam"}
                  onchange={(e) => {
                    settings.preferredSource = e.currentTarget.value as Source;
                    persistSettings();
                  }}
                  >{#each sources as source}<option value={source}
                      >{sourceNames[source]}</option
                    >{/each}</select
                >
              </div>
              <div class="setting-row">
                <div>
                  <strong>Show hidden games</strong>
                  <p>
                    Include hidden games in your library, favorites, and
                    collections. Use the eye on a cover to unhide a game.
                  </p>
                </div>
                <input
                  aria-label="Show hidden games"
                  type="checkbox"
                  checked={!!settings.showHiddenGames}
                  onchange={(event) => {
                    settings.showHiddenGames = event.currentTarget.checked;
                    persistSettings();
                  }}
                />
              </div>
              <div class="setting-row">
                <div>
                  <strong
                    >{demoMode
                      ? "Return to your library"
                      : "Explore the sample library"}</strong
                  >
                  <p>Sample games stay separate from your collection.</p>
                </div>
                <button
                  class="secondary-button"
                  onclick={() => {
                    toggleDemo();
                  }}>{demoMode ? "My library" : "Preview"}</button
                >
              </div>
            </section>
            <div class="settings-panel">
              <EmulatorSettings
                disabled={demoMode || !ready}
                profiles={settings.emulators ?? []}
                {games}
                onchange={saveEmulatorProfiles}
              />
            </div>
            <div class="settings-about">
              <span class="brand-mark"><AppLogo /></span>
              <div>
                <strong>SlopLauncher {appVersion}</strong>
                <p>Built for your collection. Stored on your PC.</p>
              </div>
            </div>
          </div>
          <div class="settings-panel">
            <ProviderSettings
              status={providers}
              onchange={(status) => (providers = status)}
            />
          </div>
        </div>
      </main>
    {:else if selected}
      {@const copy = preferredCopy(selected)}
      {@const action = launchAction(selected)}
      <main class="details-page">
        <div
          class="detail-hero"
          role="group"
          aria-label="Game background artwork"
          oncontextmenu={(event) => showGameContext(event, selected!, "hero")}
        >
          <Cover game={selected} hero />
          <div class="hero-shade"></div>
          <div class="hero-source">
            {#each selected.copies as c}<span>{sourceNames[c.source]}</span
              >{/each}
          </div>
        </div>
        <div class="details-content">
          <div class="details-heading">
            <div
              class="detail-poster"
              role="group"
              aria-label="Game box artwork"
              oncontextmenu={(event) =>
                showGameContext(event, selected!, "cover")}
            >
              <Cover game={selected} />
            </div>
            <div class="detail-identity">
              <div class="detail-tags">
                {selected.genres.slice(0, 3).join("  ·  ")}{selected.year
                  ? `  /  ${selected.year}`
                  : ""}
              </div>
              <h1>{selected.title}</h1>
              <p>
                {selected.developer ??
                  (copy?.emulation
                    ? consoleName(copy.emulation.platform)
                    : "Game details")}
              </p>
              <div class="detail-actions">
                <button
                  class="primary-button play-button"
                  disabled={!!busy || !!action.unavailable}
                  title={action.unavailable ||
                    (action.storeUrl
                      ? "Open the store to manage this owned game"
                      : undefined)}
                  onclick={() => launch(selected!)}
                  >{#if busy === `launch:${selected.id}`}<LoaderCircle
                      size={17}
                      class="spin"
                    />{:else if copy?.installed}<Play
                      size={17}
                      fill="currentColor"
                    />{:else if action.storeUrl}<ExternalLink
                      size={17}
                    />{:else}<Download size={17} />{/if}{action.label}</button
                >{#if selectedVersions.length > 1}<select
                    class="version-select"
                    aria-label="Preferred copy"
                    value={copy?.id}
                    onchange={(e) => chooseVersion(e.currentTarget.value)}
                    >{#each selectedVersions as version}{@const c =
                        version.copy}<option value={c.id}
                        >{sourceNames[c.source]}{c.installed
                          ? " · Installed"
                          : ""}</option
                      >{/each}</select
                  >{:else}<span class="launch-via"
                    >{copy?.installed ? "via" : "Source"}
                    {copy?.emulation
                      ? (settings.emulators?.find(
                          (p) => p.id === copy.emulation?.profileId,
                        )?.name ?? "Emulator")
                      : sourceNames[copy?.source]}</span
                  >{/if}
                <div class="detail-resource-actions">
                  <GameLinks
                    game={selected}
                    revision={providers.revision}
                    onerror={(message) => notify(message, true)}
                  /><button
                    class="secondary-button favorite-detail"
                    class:active={selected.favorite}
                    aria-pressed={selected.favorite}
                    onclick={() => favorite(selected!)}
                    ><Heart
                      size={17}
                      fill={selected.favorite ? "currentColor" : "none"}
                    />{selected.favorite ? "Favorited" : "Favorite"}</button
                  >
                </div>
                <GameMenu
                  onuninstall={() => void prepareUninstall(selected!)}
                  uninstallUnavailable={uninstallUnavailable(selected)}
                  gameId={selected.id}
                  copyId={copy?.id}
                  browseUnavailable={browseUnavailable(selected)}
                  ondetails={() => {
                    metadataId = String(selected?.metadataId ?? "");
                    openModal("metadata");
                  }}
                  onbrowse={() => void browseGameFiles(selected!)}
                  onartwork={() => editArtwork(selected!)}
                  onemulation={copy?.emulation
                    ? () => openModal("emulation")
                    : undefined}
                />
              </div>
            </div>
          </div>
          <div class="detail-tabs">
            <span class="active">Overview</span><button
              onclick={() => {
                metadataId = String(selected?.metadataId ?? "");
                openModal("metadata");
              }}>Edit details <ArrowUpRight size={13} /></button
            >
          </div>
          <div class="details-columns">
            <div class="details-main">
              <ReviewRatings
                game={selected}
                onmatch={() => {
                  metadataId = String(selected?.metadataId ?? "");
                  openModal("metadata");
                }}
              />
              <section class="about-section">
                <h2>About the game</h2>
                <p>
                  {selected.description ??
                    "Find this game in the catalog to add a description, artwork, and screenshots."}
                </p>
                {#if !selected.description}<button
                    class="text-button"
                    onclick={() => {
                      metadataId = String(selected?.metadataId ?? "");
                      openModal("metadata");
                    }}>Find game details <ArrowUpRight size={14} /></button
                  >{/if}
              </section>
              <GameTags
                gameId={selected.id}
                tags={selected.tags ?? []}
                customTags={games.find((g) => g.id === selectedId)?.tags ?? []}
                onchange={(values) =>
                  updateGame(selected!.id, { tags: values })}
                onfilter={(tag) => {
                  navigate("all");
                  tagFilter = tagKey(tag);
                  showFilters = true;
                }}
              />
              <GameCollections
                gameId={selected.id}
                collections={resolvedCollections}
                disabled={demoMode}
                onchange={changeMembership}
                oncreate={() => editCollection(undefined, selected!.id)}
              />
              {#if selected.screenshots.length}<section
                  class="screenshots-section"
                >
                  <h2>
                    Screenshots <span>{selected.screenshots.length}</span>
                  </h2>
                  <div class="screenshot-grid">
                    {#each selected.screenshots as image, i}<button
                        aria-label={`Open screenshot ${i + 1}`}
                        onclick={() => {
                          screenshot = image;
                          openModal("screenshot");
                        }}
                        ><img
                          src={image}
                          alt={`${selected.title} screenshot ${i + 1}`}
                          loading="lazy"
                        /></button
                      >{/each}
                  </div>
                </section>{/if}
              {#key identity(selected, achievementProvider, achievementRevision)}
                <Achievements
                  game={selected}
                  provider={achievementProvider}
                  revision={achievementRevision}
                  connected={achievementProvider === "retroachievements"
                    ? !!providers.retroAchievementsConfigured
                    : !!steamAchievementAccount?.connected &&
                      !steamAchievementAccount.reconnectRequired}
                  enabled={api.desktop && !demoMode}
                  onconnect={achievementProvider === "retroachievements"
                    ? showSettings
                    : showSources}
                  onmatch={(id) => {
                    const copy = preferredCopy(selected!);
                    if (copy) linkRetroAchievements(selected!.id, copy.id, id);
                  }}
                  busy={enrichmentProgress.active.some(
                    (j) =>
                      j.provider === achievementProvider &&
                      j.title === selected?.title,
                  )}
                />
              {/key}
              <section class="notes-section">
                <h2>Your notes</h2>
                <textarea
                  aria-label="Your notes"
                  placeholder="A build to try, a quest to finish, something to remember…"
                  value={selected.notes}
                  oninput={(e) =>
                    updateNotes(selected!.id, e.currentTarget.value)}
                  onchange={persist}></textarea><span class="field-hint"
                  >{demoMode
                    ? "Preview changes are temporary"
                    : saveStatus === "error"
                      ? "Changes have not been saved"
                      : "Changes save automatically on this PC"}</span
                >
                {#if !demoMode && saveStatus === "error"}<button
                    class="text-button"
                    onclick={persist}>Retry saving</button
                  >{/if}
              </section>
            </div>
            <aside class="game-facts">
              <CompletionTimes game={selected} />
              <h2>In your library</h2>
              <div class="fact">
                <span>Availability</span><strong
                  >{copy?.installed ? "Installed" : "Not installed"}</strong
                >
              </div>
              <div class="fact">
                <span>Developer</span><strong
                  >{selected.developer ?? "Not available"}</strong
                >
              </div>
              <div class="fact">
                <span>Publisher</span><strong
                  >{selected.publisher ?? "Not available"}</strong
                >
              </div>
              <div class="fact">
                <span>Release year</span><strong
                  >{selected.year ?? "Not available"}</strong
                >
              </div>
              <label class="fact status-fact"
                ><span>Your progress</span><select
                  aria-label="Your progress"
                  value={selected.status}
                  onchange={(e) =>
                    updateGame(selected!.id, {
                      status: e.currentTarget.value as Game["status"],
                    })}
                  ><option value="unplayed">Not started</option><option
                    value="playing">Playing</option
                  ><option value="completed">Completed</option><option
                    value="on-hold">On hold</option
                  ></select
                ></label
              >{#if copy?.installPath}<div class="fact install-fact">
                  <span>Install location</span><code>{copy.installPath}</code>
                </div>{/if}
              <div class="local-note">
                <Monitor size={14} />Your library stays on your PC.
              </div>
              {#if artworkCredits.length || currentResult(selected, "igdb", providers.revision)?.data || currentResult(selected, "steamgriddb", providers.revision)?.data || currentResult(selected, "screenscraper", providers.revision)?.data}
                <details class="metadata-credits">
                  <summary>Metadata & artwork credits</summary>
                  {#each artworkCredits as credit}
                    <a
                      href={credit.page}
                      onclick={(e) => {
                        e.preventDefault();
                        void openResource(credit.page!);
                      }}
                      >{credit.slot === "cover"
                        ? "Selected box art"
                        : "Selected background"}{credit.author
                        ? ` by ${credit.author}`
                        : ""}
                      <ArrowUpRight size={12} /></a
                    >
                  {/each}
                  {#if currentResult(selected, "screenscraper", providers.revision)?.data?.url}<a
                      href={currentResult(
                        selected,
                        "screenscraper",
                        providers.revision,
                      )!.data!.url}
                      onclick={(e) => {
                        e.preventDefault();
                        void openResource(
                          currentResult(
                            selected!,
                            "screenscraper",
                            providers.revision,
                          )!.data!.url!,
                        );
                      }}
                      >Console information & artwork from ScreenScraper <ArrowUpRight
                        size={12}
                      /></a
                    >{/if}
                  {#if currentResult(selected, "igdb", providers.revision)?.data?.url}<a
                      href={currentResult(selected, "igdb", providers.revision)!
                        .data!.url}
                      onclick={(e) => {
                        e.preventDefault();
                        void openResource(
                          currentResult(selected!, "igdb", providers.revision)!
                            .data!.url!,
                        );
                      }}
                      >Game information from IGDB <ArrowUpRight size={12} /></a
                    >{/if}
                  {#if currentResult(selected, "steamgriddb", providers.revision)?.data?.url}<a
                      href={currentResult(
                        selected,
                        "steamgriddb",
                        providers.revision,
                      )!.data!.artworkPage ??
                        currentResult(
                          selected,
                          "steamgriddb",
                          providers.revision,
                        )!.data!.url}
                      onclick={(e) => {
                        e.preventDefault();
                        const data = currentResult(
                          selected!,
                          "steamgriddb",
                          providers.revision,
                        )!.data!;
                        void openResource(data.artworkPage ?? data.url!);
                      }}
                      >SteamGridDB match · {currentResult(
                        selected,
                        "steamgriddb",
                        providers.revision,
                      )!.data!.author ?? "SteamGridDB contributors"}
                      <ArrowUpRight size={12} /></a
                    >{/if}
                </details>
              {/if}
            </aside>
          </div>
        </div>
      </main>
    {:else}
      <main
        class="library-page"
        class:compact-library={!settings.comfortableLibrary}
      >
        <div class="library-heading">
          <div>
            <div class="page-eyebrow">YOUR COLLECTION</div>
            <h1>
              <span class="heading-title" {title}>{title}</span><span
                class="heading-count">{filtered.length}</span
              >
            </h1>
            <p class="library-subtitle">
              {isCollectionFilter(filter)
                ? "A collection from across your library."
                : filter === "all"
                  ? `${installedCount} installed · ${sources.filter((s) => browsableGames.some((g) => g.copies.some((c) => c.source === s))).length} sources`
                  : filter === "favorites"
                    ? "The ones you keep coming back to."
                    : filter === "recent"
                      ? "Games with confirmed play history."
                      : filter === "hidden"
                        ? "Use the eye on a cover to return a game to your library."
                        : `Your ${sourceNames[filter]} collection`}
            </p>
            {#if activeCollection}<button
                class="text-button collection-edit-button"
                onclick={() => editCollection(activeCollection)}
                ><Settings2 size={14} />Edit collection</button
              >{/if}
          </div>
          <div class="library-search">
            <Search size={17} /><input
              bind:this={searchInput!}
              bind:value={query}
              placeholder="Search your games"
              aria-label="Search your games"
            /><kbd>Ctrl K</kbd>{#if query}<button
                aria-label="Clear search"
                onclick={() => (query = "")}><X size={14} /></button
              >{/if}
          </div>
        </div>
        <div class="library-toolbar">
          <div class="filter-pills">
            <button
              class:chosen={!installedOnly}
              onclick={() => (installedOnly = false)}>All games</button
            ><button
              class:chosen={installedOnly}
              onclick={() => (installedOnly = true)}
              ><span class="tiny-dot"></span>Installed</button
            ><button
              class="filter-toggle"
              class:chosen={showFilters}
              aria-expanded={showFilters}
              onclick={() => (showFilters = !showFilters)}
              ><SlidersHorizontal size={14} />Filters{#if tagFilter}<span
                  class="filter-count">1</span
                >{/if}</button
            >
          </div>
          <div class="library-layout-controls">
            <label class="sort-control"
              ><span>Group by</span><select
                aria-label="Group library"
                value={groupingMode}
                onchange={(event) => {
                  settings.libraryGrouping = event.currentTarget
                    .value as GroupMode;
                  persistSettings();
                }}
                >{#each groupOptions as [value, label]}<option {value}
                    >{label}</option
                  >{/each}</select
              ><ChevronDown size={12} /></label
            >
            {#if groupingMode === "smart"}<button
                class="text-button"
                onclick={() => openModal("grouping")}>Edit smart groups</button
              >{/if}
            <div class="sort-control">
              <span>Sort by</span><select
                aria-label="Sort library"
                bind:value={sort}
                onchange={() => {
                  settings.librarySort = sort;
                  persistSettings();
                }}
                >{#each sortOptions as [value, label]}<option {value}
                    >{label}</option
                  >{/each}</select
              ><ChevronDown size={12} />
            </div>
          </div>
        </div>
        {#if showFilters}<div class="expanded-filters">
            <label
              >Tag<select aria-label="Tag" bind:value={tagFilter}
                ><option value="">All tags</option>{#each tags as g}<option
                    value={tagKey(g)}>{g}</option
                  >{/each}</select
              ></label
            ><label class="size-control"
              >Cover size<input
                aria-label="Cover size"
                type="range"
                min="140"
                max="250"
                step="10"
                bind:value={settings.coverSize}
                onchange={persistSettings}
              /></label
            >{#if tagFilter}<button
                class="text-button"
                onclick={() => (tagFilter = "")}
                >Clear filters<X size={13} /></button
              >{/if}
          </div>{/if}
        {#if loadError}<div class="empty-state" role="alert">
            <Info size={28} />
            <h2>Your library could not open</h2>
            <p>
              Your saved data has been kept. Retry opening it before making
              changes.
            </p>
            <button class="primary-button" onclick={() => void loadLibrary()}
              >Retry opening library</button
            >
          </div>
        {:else if !ready}<div class="empty-state">
            <LoaderCircle size={28} class="spin" />
            <h2>Opening your library</h2>
          </div>{:else if !filtered.length}<div class="empty-state">
            <span class="empty-icon"
              ><Gamepad2 size={31} strokeWidth={1.3} /></span
            >
            <h2>
              {filter === "hidden"
                ? hiddenCount
                  ? "No matching hidden games"
                  : "No hidden games"
                : activeCollection &&
                    !totalFolderCounts.get(activeCollection.id)
                  ? "Your collection is ready"
                  : games.length
                    ? "No matching games"
                    : "Your library is ready"}
            </h2>
            <p>
              {filter === "hidden"
                ? hiddenCount
                  ? "Try another search or adjust your filters."
                  : "Games you hide will appear here. Your other games remain in the library."
                : activeCollection &&
                    !totalFolderCounts.get(activeCollection.id)
                  ? activeCollection.rule
                    ? "New matching games will appear here automatically. Edit this collection to save its current matches as a snapshot."
                    : "Choose games from your library to add to this collection."
                  : games.length
                    ? hiddenCount && !settings.showHiddenGames
                      ? "Try another search or enable Show hidden games in Settings."
                      : "Try another search or adjust your filters."
                    : "Find installed games, connect your accounts, or add a game of your own."}
            </p>
            <div>
              {#if filter === "hidden" && !hiddenCount}<button
                  class="secondary-button"
                  onclick={() => navigate("all")}>Back to library</button
                >
              {:else if activeCollection && !totalFolderCounts.get(activeCollection.id)}<button
                  class="primary-button"
                  onclick={() => editCollection(activeCollection)}
                  ><Plus size={15} />{activeCollection.rule
                    ? "Edit matching collection"
                    : "Add games to collection"}</button
                >
              {:else if games.length}<button
                  class="secondary-button"
                  onclick={() => {
                    query = "";
                    tagFilter = "";
                    installedOnly = false;
                  }}>Clear filters</button
                >{:else}<button
                  class="primary-button"
                  onclick={scan}
                  disabled={!!busy || !api.desktop}
                  ><RefreshCw
                    size={15}
                    class={busy === "scan" ? "spin" : ""}
                  />Find installed games</button
                ><button
                  class="secondary-button"
                  onclick={() => openModal("add")}
                  ><Plus size={15} />Add game</button
                >{/if}
              {#if filter !== "hidden" && hiddenCount && !settings.showHiddenGames}<button
                  class="text-button"
                  onclick={showSettings}>Show hidden games in Settings</button
                >{/if}
            </div>
            {#if !games.length}<button class="text-button" onclick={toggleDemo}
                >Explore a sample library<ArrowUpRight size={13} /></button
              >{/if}
          </div>{:else}{#key filter}<VirtualGrid
              games={filtered}
              groups={grouped}
              onsavegroup={demoMode
                ? undefined
                : (id) =>
                    collectionFromGroup(
                      id,
                      undefined,
                      undefined,
                      grouped.find((g) => g.id === id)?.games.map((g) => g.id),
                    )}
              coverSize={settings.coverSize}
              compact={!settings.comfortableLibrary}
              onopen={openGame}
              onfavorite={favorite}
              onhide={hideGame}
              onlaunch={launch}
              oncontext={showGameContext}
              versionCount={(game) =>
                settings.hideDuplicateGames
                  ? versionsIndex.versions(game.id).length
                  : game.copies.length}
              initialScroll={libraryScroll}
              onscroll={(value) => {
                if (!selectedId && page === "library") libraryScroll = value;
              }}
            />{/key}{/if}
        <footer class="library-footer">
          <span
            >{demoMode
              ? "Sample games · artwork from Steam"
              : saveStatus === "error"
                ? "Changes could not be saved"
                : busy === "scan"
                  ? "Reading local game records…"
                  : "Changes save automatically on this PC"}</span
          ><span
            >{filtered.length}
            {filtered.length === 1 ? "game" : "games"}<span class="footer-dot"
              >·</span
            >{demoMode ? "Preview" : api.desktop ? "Desktop" : "Browser"}</span
          >
        </footer>
      </main>
    {/if}
    {#if ready && !demoMode && api.desktop}
      <EnrichmentStatus
        progress={enrichmentProgress}
        syncing={accountStates
          .filter((a) => a.phase === "syncing")
          .map((a) => a.source)}
        downloads={artworkDownloads}
        catalog={$artworkCatalogState}
        enabled={settings.autoFillGaps !== false}
        missing={gapCount}
      />
    {/if}
  </div>
</div>

{#if contextMenu && contextGame}
  <GameMenu
    gameId={contextGame.id}
    copyId={preferredCopy(contextGame)?.id}
    anchor={contextMenu.anchor}
    artworkOnly={contextMenu.slot}
    browseUnavailable={browseUnavailable(contextGame)}
    uninstallUnavailable={uninstallUnavailable(contextGame)}
    ondetails={() => editGameDetails(contextGame!)}
    onbrowse={() => void browseGameFiles(contextGame!)}
    onartwork={() => editArtwork(contextGame!, contextMenu?.slot ?? "cover")}
    onresetartwork={() =>
      chooseArtwork(contextGame!, contextMenu?.slot ?? "cover", null)}
    onemulation={preferredCopy(contextGame)?.emulation
      ? () => {
          openGame(contextGame!);
          openModal("emulation");
        }
      : undefined}
    onuninstall={() => void prepareUninstall(contextGame!)}
    onclose={() => (contextMenu = null)}
  />
{/if}

<dialog
  bind:this={dialog!}
  oncancel={() => closeModal()}
  onclose={() => {
    modal = null;
  }}
  class:media-dialog={modal === "screenshot"}
  class:artwork-dialog={modal === "artwork"}
  class:grouping-dialog={modal === "grouping"}
  aria-labelledby="dialog-title"
>
  {#if modal}
    <div class="dialog-header">
      <h2 id="dialog-title">
        {modal === "add"
          ? "Add a game"
          : modal === "metadata"
            ? "Game details"
            : modal === "uninstall"
              ? "Uninstall game"
              : modal === "remove"
                ? "Remove game"
                : modal === "emulation"
                  ? "Emulator setup"
                  : modal === "collection"
                    ? collections.some((c) => c.id === collectionDraft.id)
                      ? "Edit collection"
                      : "New collection"
                    : modal === "artwork"
                      ? "Change Artwork"
                      : modal === "grouping"
                        ? "Smart grouping"
                        : "Screenshot"}
      </h2>
      <button class="icon-button" aria-label="Close dialog" onclick={closeModal}
        ><X size={19} /></button
      >
    </div>
    {#if modal === "artwork" && artworkGame}
      {#key artworkIdentity(artworkGame)}
        <ArtworkPicker
          game={artworkGame}
          revision={providers.revision}
          gridConfigured={providers.steamGridDbConfigured}
          initialSlot={artworkSlot}
          onselect={(slot, asset) => chooseArtwork(artworkGame!, slot, asset)}
        />
      {/key}
    {:else if modal === "grouping"}
      <SmartGroupEditor
        initial={groupingEditorDraft ?? settings.smartGroups ?? []}
        games={presentedGames.filter(
          (g) => settings.showHiddenGames || !g.hidden,
        )}
        onsave={saveSmartGroups}
        oncollection={demoMode
          ? undefined
          : (rules, id, draft) => {
              groupingEditorDraft = draft;
              collectionReturnToGroups = true;
              collectionFromGroup(id, rules, "smart");
            }}
        oncancel={closeModal}
      />
    {:else if modal === "collection"}
      <CollectionEditor
        initial={collectionDraft}
        games={presentedGames}
        now={clockTime}
        existing={collections.some((c) => c.id === collectionDraft.id)}
        onsave={commitCollection}
        ondelete={deleteCollection}
        oncancel={closeModal}
      />
    {:else if modal === "add"}
      <div class="add-game-modes" role="group" aria-label="Game type">
        <button
          class:chosen={addMode === "pc"}
          aria-pressed={addMode === "pc"}
          onclick={() => (addMode = "pc")}>PC game</button
        ><button
          class:chosen={addMode === "emulated"}
          aria-pressed={addMode === "emulated"}
          onclick={() => (addMode = "emulated")}>Emulated game</button
        >
      </div>
      {#if addMode === "emulated"}<EmulatedGameForm
          profiles={settings.emulators ?? []}
          onsubmit={addEmulatedGame}
          oncancel={closeModal}
          onsetup={() => showSettings()}
        />{:else}<form onsubmit={addGame}>
          <p class="dialog-description">
            Give a game installed anywhere on your PC a home in your library.
          </p>
          <label
            >Game title<input
              bind:value={addTitle}
              required
              maxlength="300"
              placeholder="Game name"
            /></label
          ><label
            >Game executable
            <div class="input-with-button">
              <input
                bind:value={addPath}
                required
                placeholder="C:\Games\My Game\game.exe"
              /><button type="button" class="secondary-button" onclick={browse}
                ><FolderOpen size={16} />Browse</button
              >
            </div></label
          ><label
            >Steam app ID <span class="optional">optional</span><input
              bind:value={addSteamId}
              inputmode="numeric"
              placeholder="e.g. 1245620"
            /></label
          >
          <p class="field-hint">
            Use the number in its Steam store URL to fetch artwork and details.
            This doesn’t add Steam ownership.
          </p>
          {#if formError}<p class="form-error" role="alert">{formError}</p>{/if}
          <div class="dialog-footer">
            <button type="button" class="secondary-button" onclick={closeModal}
              >Cancel</button
            ><button class="primary-button" disabled={!!busy || !api.desktop}
              >{#if busy === "add"}<LoaderCircle
                  size={15}
                  class="spin"
                />{/if}Add to library</button
            >
          </div>
        </form>
      {/if}
    {:else if modal === "emulation" && selected && preferredCopy(selected)?.emulation}
      {@const emu = preferredCopy(selected)!.emulation!}
      <EmulatedGameForm
        profiles={settings.emulators ?? []}
        initial={{
          title: selected.title,
          profileId: emu.profileId,
          gamePath: emu.gamePath,
        }}
        onsubmit={saveEmulatedGame}
        oncancel={closeModal}
        onsetup={() => showSettings()}
      />
    {:else if modal === "metadata"}<form
        onsubmit={(event) => {
          event.preventDefault();
          searchMetadata();
        }}
      >
        <p class="dialog-description">
          Choose the correct game to load its artwork, description, and
          screenshots. Your store copy stays the same.
        </p>
        <label
          >Find a game
          <div class="input-with-button">
            <input
              aria-label="Find a game"
              bind:value={metadataQuery}
              oninput={() => {
                metadataResults = [];
                metadataSearched = false;
              }}
              placeholder="Search by game title"
            /><button
              type="button"
              class="secondary-button"
              onclick={searchMetadata}
              disabled={!!busy || demoMode || !api.desktop}
              >{#if busy === "search-metadata"}<LoaderCircle
                  size={15}
                  class="spin"
                />{:else}<Search size={15} />{/if}Search</button
            >
          </div></label
        >
        {#if metadataResults.length}<div class="metadata-results">
            {#each metadataResults as match}<button
                type="button"
                disabled={!!busy}
                onclick={() => applyMatch(match.appId)}
                ><span>{match.title}</span><ArrowUpRight size={14} /></button
              >{/each}
          </div>{:else if metadataSearched}<p class="field-hint">
            No matching games found. Try a shorter title or a different
            spelling.
          </p>{/if}
        <details class="advanced-metadata">
          <summary>Link by Steam app ID</summary>
          <label
            >Steam app ID<input
              bind:value={metadataId}
              inputmode="numeric"
              placeholder="Number from the game’s Steam URL"
            /></label
          >
          <button
            type="button"
            class="secondary-button"
            disabled={!!busy || demoMode || !api.desktop || !metadataId}
            onclick={() => applyMatch(Number(metadataId))}>Apply app ID</button
          >
        </details>
        {#if selected}<details class="advanced-metadata">
            <summary>Provider matches</summary>
            <p class="field-hint">
              Set a specific listing when a title is ambiguous. Clear an ID to
              return to automatic matching.
            </p>
            <label
              >IGDB game ID<input
                aria-label="IGDB game ID"
                inputmode="numeric"
                value={selected.providerIds?.igdb ?? ""}
                onchange={(e) => setProviderId("igdb", e.currentTarget.value)}
              /></label
            >
            <label
              >SteamGridDB game ID<input
                aria-label="SteamGridDB game ID"
                inputmode="numeric"
                value={selected.providerIds?.steamgriddb ?? ""}
                onchange={(e) =>
                  setProviderId("steamgriddb", e.currentTarget.value)}
              /></label
            >
            <label
              >HowLongToBeat game ID<input
                aria-label="HowLongToBeat game ID"
                inputmode="numeric"
                value={selected.providerIds?.hltb ?? ""}
                onchange={(e) => setProviderId("hltb", e.currentTarget.value)}
              /></label
            >
            <label
              >Metacritic game slug<input
                aria-label="Metacritic game slug"
                placeholder="e.g. portal-2"
                value={selected.providerIds?.metacriticSlug ?? ""}
                onchange={(e) => {
                  const value = e.currentTarget.value.trim();
                  if (value && !/^[a-zA-Z0-9_-]{1,200}$/.test(value)) {
                    notify(
                      "Enter the slug from the Metacritic game URL.",
                      true,
                    );
                    return;
                  }
                  updateGame(selected!.id, {
                    providerIds: {
                      ...selected!.providerIds,
                      metacriticSlug: value || undefined,
                    },
                  });
                }}
              /></label
            >
            <label
              >OpenCritic game path<input
                aria-label="OpenCritic game path"
                placeholder="e.g. 10181/hades"
                value={selected.providerIds?.opencriticPath ?? ""}
                onchange={(e) => {
                  const value = e.currentTarget.value.trim();
                  if (
                    value &&
                    !/^[1-9]\d*\/[a-zA-Z0-9_-]{1,200}$/.test(value)
                  ) {
                    notify(
                      "Enter the number and slug after /game/ in the OpenCritic URL.",
                      true,
                    );
                    return;
                  }
                  updateGame(selected!.id, {
                    providerIds: {
                      ...selected!.providerIds,
                      opencriticPath: value || undefined,
                    },
                  });
                }}
              /></label
            >
            <label
              >GOG review product ID<input
                aria-label="GOG review product ID"
                inputmode="numeric"
                value={selected.providerIds?.gogReviews ?? ""}
                onchange={(e) =>
                  setProviderId("gogReviews", e.currentTarget.value)}
              /></label
            >
            <p class="field-hint">
              Review matches fill automatically when available. Override only if
              a source matched the wrong edition. Metacritic uses the game slug
              from its URL; OpenCritic uses the number and slug after /game/.
              Clear an override to restore automatic matching.
            </p>
            {#if selected.copies.some((c) => c.emulation)}
              {#if currentResult(selected, "screenscraper", providers.revision)?.message}<p
                  class="field-hint"
                >
                  ScreenScraper: {currentResult(
                    selected,
                    "screenscraper",
                    providers.revision,
                  )!.message}
                </p>{/if}
              <label
                >ScreenScraper game ID<input
                  aria-label="ScreenScraper game ID"
                  inputmode="numeric"
                  value={selected.providerIds?.screenscraper ?? ""}
                  onchange={(e) =>
                    setProviderId("screenscraper", e.currentTarget.value)}
                /></label
              >
              <label
                >ScreenScraper system ID override<input
                  aria-label="ScreenScraper system ID"
                  inputmode="numeric"
                  value={selected.providerIds?.screenScraperSystem ?? ""}
                  placeholder="Automatic for supported consoles"
                  onchange={(e) =>
                    setProviderId("screenScraperSystem", e.currentTarget.value)}
                /></label
              >
              <p class="field-hint">
                Use a game ID when several versions share a title. Other
                consoles also need a system ID from ScreenScraper.
              </p>
            {/if}
            <label
              >Nexus Mods game domain<input
                aria-label="Nexus Mods game domain"
                value={selected.providerIds?.nexus ?? ""}
                placeholder="Game name from its Nexus URL"
                onchange={(e) => setNexusDomain(e.currentTarget.value)}
              /></label
            >
            <p class="field-hint">
              The Nexus button appears only when its catalog confirms published
              mods.
            </p>
          </details>{/if}
        {#if selected}<label
            >Display title<input
              value={selected.title}
              onchange={(e) =>
                updateGame(selected!.id, {
                  title: e.currentTarget.value.trim() || selected!.title,
                  titleOverride: true,
                })}
            /></label
          >{/if}{#if formError}<p class="form-error" role="alert">
            {formError}
          </p>{/if}
        <div class="dialog-footer">
          <button type="button" class="secondary-button" onclick={closeModal}
            >Done</button
          >
        </div>
      </form>
      {#if selected?.copies.every((c) => c.source === "manual" || c.source === "emulated")}
        <div class="remove-entry">
          <button class="text-button" onclick={() => openModal("remove")}
            >Remove from library</button
          ><span class="field-hint">Your game files stay on disk.</span>
        </div>
      {/if}
    {:else if modal === "uninstall" && uninstallRequest}
      <p>Uninstall <strong>{uninstallRequest.plan.title}</strong>?</p>
      <p class="muted">{uninstallRequest.plan.message}</p>
      {#if uninstallRequest.plan.source}<p class="field-hint">
          Version: {sourceNames[uninstallRequest.plan.source as Source] ??
            uninstallRequest.plan.source}
        </p>{/if}
      {#if uninstallRequest.plan.installPath}<code class="uninstall-path"
          >{uninstallRequest.plan.installPath}</code
        >{/if}
      <p class="field-hint">
        Your library entry, notes, favorites, collections, and other versions
        will be kept.
      </p>
      {#if formError}<p class="form-error" role="alert">{formError}</p>{/if}
      <div class="dialog-actions">
        <button class="secondary-button" onclick={closeModal}>Cancel</button
        ><button
          class="danger-button"
          disabled={!!busy}
          onclick={() => void confirmUninstall()}
          >Open {uninstallRequest.plan.destination}</button
        >
      </div>
    {:else if modal === "remove"}
      <p class="dialog-description">
        Remove {selected?.title} and its notes from this library? The installed game
        files stay on disk.
      </p>
      <div class="dialog-footer">
        <button class="secondary-button" onclick={closeModal}>Cancel</button
        ><button class="primary-button" onclick={removeManualGame}
          >Remove entry</button
        >
      </div>
    {:else if modal === "screenshot"}<img
        class="full-screenshot"
        src={screenshot}
        alt="Game screenshot"
      />{/if}
  {/if}
</dialog>
{#if toast}<div class:error={toastError} class="toast" role="status">
    {#if toastError}<Info size={18} />{:else}<CheckCircle2
        size={18}
      />{/if}<span>{toast}</span><button
      aria-label="Dismiss notification"
      onclick={() => (toast = "")}><X size={16} /></button
    >
  </div>{/if}
