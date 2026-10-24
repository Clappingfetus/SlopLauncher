import type { ThemeId } from "./themes";
import type { EmulatedCopy, EmulatorProfile } from "./emulation";
export const sources = [
  "steam",
  "epic",
  "amazon",
  "gog",
  "itch",
  "emulated",
  "manual",
] as const;
export type Source = (typeof sources)[number];
export const sourceNames: Record<Source, string> = {
  steam: "Steam",
  epic: "Epic Games",
  amazon: "Amazon Games",
  gog: "GOG",
  itch: "itch.io",
  emulated: "Emulated",
  manual: "Manually added",
};
export interface Copy {
  id: string;
  source: Source;
  sourceId: string;
  installed: boolean;
  installSizeBytes?: number;
  executable?: string;
  installPath?: string;
  launchUrl?: string;
  available?: boolean;
  ownership?: { accountId: string; verifiedAt: number };
  storePage?: StorePage;
  emulation?: EmulatedCopy;
}
export interface StorePage {
  source: Source;
  sourceId: string;
  url: string;
}
export interface Game {
  metadataScan?: { identity: string; attempted: ProviderId[] };
  /** Derived from current duplicate copies; not persisted as personal metadata. */
  reviewSummary?: import("./ratings").RatingRow[];
  id: string;
  title: string;
  titleOverride?: boolean;
  storeMetadata?: StoreCatalogRecord[];
  /** Presentation only: ordered alternatives if a remote image fails. */
  artworkFallbacks?: Partial<Record<"cover" | "hero", string[]>>;
  copies: Copy[];
  cover?: string;
  hero?: string;
  description?: string;
  developer?: string;
  publisher?: string;
  genres: string[];
  tags?: string[];
  year?: number;
  releasedAt?: number;
  screenshots: string[];
  favorite: boolean;
  hidden?: boolean;
  favoriteUpdatedAt?: number;
  hiddenUpdatedAt?: number;
  status: "unplayed" | "playing" | "completed" | "on-hold";
  notes: string;
  addedAt: number;
  /** Legacy local play activity. Steam snapshots never overwrite this field. */
  lastPlayed?: number;
  steamActivity?: import("./play-activity").SteamActivity[];
  launcherActivity?: import("./play-activity").LauncherActivity;
  /** Duplicate-aware presentation only; native persistence retains raw histories. */
  activitySummary?: import("./play-activity").ActivitySummary;
  preferredCopy?: string;
  metadataId?: number;
  metadataLocked?: boolean;
  artwork?: Partial<Record<"cover" | "hero", ArtworkChoice>>;
  providerIds?: {
    igdb?: number;
    steamgriddb?: number;
    hltb?: number;
    pcgwTitle?: string;
    metacriticSlug?: string;
    opencriticPath?: string;
    gogReviews?: number;
    nexus?: string;
    screenscraper?: number;
    screenScraperSystem?: number;
  };
  enrichment?: Partial<Record<ProviderId, EnrichmentRecord>>;
}
export interface StoreCatalogRecord {
  coverKind?: "portrait";
  source: Source;
  sourceId: string;
  title: string;
  rawTitle?: string;
  cover?: string;
  hero?: string;
  checkedAt: number;
}
export interface ArtworkAsset {
  url: string;
  author?: string;
  page?: string;
}
export interface ArtworkChoice extends ArtworkAsset {
  source: "steamgriddb" | "igdb" | "screenscraper" | "store";
  identity: string;
  selectedAt: number;
}
export const providerIds = [
  "retroachievements",
  "achievements",
  "steam",
  "pcgw",
  "hltb",
  "igdb",
  "steamgriddb",
  "nexus",
  "stores",
  "screenscraper",
  "reviews-steam",
  "reviews-gog",
  "reviews-metacritic",
  "reviews-opencritic",
  "reviews-itch",
] as const;
export type ProviderId = (typeof providerIds)[number];
export interface ReviewRating {
  source: "steam" | "gog" | "metacritic" | "opencritic" | "itch";
  kind: "users" | "critics";
  value: number;
  scale: number;
  count: number;
  url: string;
  platform?: string;
}
export interface ProviderData {
  rating?: ReviewRating;
  reviewLinks?: { metacritic?: string; opencritic?: string };
  achievements?: AchievementData;
  releasedAt?: number;
  covers?: ArtworkAsset[];
  heroes?: ArtworkAsset[];
  storePages?: StorePage[];
  id?: number;
  url?: string;
  title?: string;
  hltbId?: number;
  nexusDomains?: string[];
  nexusDomain?: string;
  modCount?: number;
  cover?: string;
  hero?: string;
  summary?: string;
  developer?: string;
  publisher?: string;
  year?: number;
  genres?: string[];
  tags?: string[];
  screenshots?: string[];
  mainHours?: number;
  extraHours?: number;
  completionistHours?: number;
  sampleCount?: number;
  author?: string;
  artworkPage?: string;
}
export interface AchievementData {
  source: "steam" | "retroachievements";
  title?: string;
  consoleName?: string;
  appId: number;
  accountId: string;
  progressAvailable: boolean;
  items: {
    id: string;
    name: string;
    description: string;
    hidden: boolean;
    unlocked?: boolean;
    unlockedAt?: number;
    hardcoreUnlocked?: boolean;
    hardcoreUnlockedAt?: number;
    points?: number;
  }[];
}
export interface ProviderResult {
  status: "ready" | "not-found" | "needs-match" | "unavailable" | "needs-key";
  checkedAt: number;
  nextCheckAt: number;
  message?: string;
  data?: ProviderData;
}
export interface EnrichmentRecord extends ProviderResult {
  fingerprint: string;
  dataCheckedAt?: number;
}
export interface ProviderStatus {
  retroAchievementsConfigured?: boolean;
  retroAchievementsUsername?: string;
  screenScraperConfigured?: boolean;
  revision: number;
  steamGridDbConfigured: boolean;
  igdbConfigured: boolean;
  igdbClientId?: string;
  warning?: string;
}
export interface ProviderConfigInput {
  retroAchievements?: { username?: string; apiKey?: string };
  clearRetroAchievements?: boolean;
  screenScraper?: {
    developerId?: string;
    developerPassword?: string;
    username?: string;
    password?: string;
  };
  clearScreenScraper?: boolean;
  steamGridDbKey?: string;
  igdbClientId?: string;
  igdbClientSecret?: string;
  clearSteamGridDb?: boolean;
  clearIgdb?: boolean;
}
export interface VersionPreference {
  gameId: string;
  copyId: string;
  selectedAt: number;
}
export interface Settings {
  homeRowOrder?: string[];
  hideDuplicateGames?: boolean;
  preferredSource?: Source;
  versionPreferences?: Record<string, VersionPreference>;
  librarySort?: string;
  libraryGrouping?: GroupMode;
  smartGroups?: SmartGroup[];
  collections?: GameCollection[];
  comfortableLibrary?: boolean;
  coverSize: number;
  theme: ThemeId;
  startInstalled: boolean;
  showHiddenGames?: boolean;
  autoFillGaps?: boolean;
  librarySyncHours?: number;
  steamId: string;
  libraryFolders?: LibraryFolders;
  emulators?: EmulatorProfile[];
}
export type GroupMode =
  | "none"
  | "tags"
  | "size"
  | "release"
  | "played"
  | "added"
  | "hltb"
  | "rating"
  | "smart";
export type RuleField =
  "tags" | "size" | "release" | "played" | "added" | "hltb" | "rating";
export interface SmartCondition {
  id: string;
  field: RuleField;
  op: string;
  value?: string;
  upper?: string;
}
export interface SmartGroup {
  id: string;
  name: string;
  match: "all" | "any";
  conditions: SmartCondition[];
}
export interface LibraryGroup {
  id: string;
  name: string;
  games: Game[];
}
export interface CollectionRule {
  mode: Exclude<GroupMode, "none">;
  groupId: string;
  groups: SmartGroup[];
}
export interface GameCollection {
  rule?: CollectionRule;
  id: string;
  name: string;
  gameIds: string[];
}
export type LibraryFolders = Partial<Record<AccountSource, string[]>>;
export interface Library {
  games: Game[];
  settings: Settings;
}
export interface ScanReport {
  games: Game[];
  messages: string[];
}
export type AccountSource = Exclude<Source, "manual" | "emulated">;
export interface AccountStatus {
  source: AccountSource;
  connected: boolean;
  displayName?: string;
  accountId?: string;
  revision: number;
  lastSync?: number;
  nextRetryAt?: number;
  ownedCount?: number;
  reconnectRequired: boolean;
  error?: string;
}
export interface LoginTicket {
  id: string;
  expiresAt: number;
}
export interface AccountSync extends ScanReport {
  source: AccountSource;
  revision: number;
  account: AccountStatus;
}
export interface StoreMetadata {
  title: string;
  description: string;
  developer: string;
  publisher: string;
  genres: string[];
  year?: number;
  cover: string;
  hero: string;
  screenshots: string[];
}
export const defaultSettings: Settings = {
  coverSize: 176,
  theme: "dark",
  startInstalled: false,
  showHiddenGames: false,
  autoFillGaps: true,
  librarySyncHours: 6,
  steamId: "",
  libraryFolders: {},
};
