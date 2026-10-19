export interface GameRecord {
  id: string;
  title: string;
  exePath: string | null;
  installPath: string;
  source: "steam" | "epic" | "manual" | string;
  sourceId: string | null;
  favorite: boolean;
  coverPath: string | null;
  coverDataUrl: string | null;
  addedAt: string;
  lastPlayed: string | null;
  totalSeconds: number;
  launchCount: number;
}

export type TrainerCatalogStatus = "available" | "unsupported" | "unavailable" | "unknown";

export interface TrainerCatalogResult {
  title: string;
  aliases?: string[];
  platforms: string[];
  cheatCount: number | null;
  status: TrainerCatalogStatus;
}

export interface CheatEntry {
  id: string;
  title: string;
  address: string;
  variableType: string;
  value: string;
  showAsHex: boolean;
}

export interface CheatTable {
  sourceFileName: string;
  importedAt: string;
  skippedEntries: number;
  entries: CheatEntry[];
}

export interface OnlineCheatTableResult {
  gameTitle: string;
  fileName: string;
  path: string;
  repository: string;
  repositoryUrl: string;
  blobSha: string;
  sizeBytes: number;
  stars: number;
}

export interface ScanResult {
  found: number;
  added: number;
  updated: number;
  steamFound: number;
  epicFound: number;
  gogFound: number;
  riotFound: number;
  emulatorFound: number;
  deviceFound: number;
  warnings: string[];
}

export interface LaunchResult {
  started: boolean;
  tracking: boolean;
  message: string;
}

export interface Stats {
  gameCount: number;
  favoriteCount: number;
  playedGameCount: number;
  totalSeconds: number;
  launchCount: number;
  last7DaysSeconds: number;
  topGame: string | null;
}

export interface PlatformAchievement {
  gameId: string;
  appId: string;
  gameTitle: string;
  achievementId: string;
  title: string;
  description: string;
  unlocked: boolean;
  unlockTime: number | null;
  iconUrl: string | null;
}

export interface SteamAchievementStatus {
  steamId: string | null;
  apiKeyConfigured: boolean;
  lastScanAt: string | null;
}

export interface PlatformAchievementScanResult {
  scannedGames: number;
  updatedAchievements: number;
  skippedGames: number;
  skippedRecently: boolean;
}

export interface ScreenshotRecord {
  id: number;
  gameId: string;
  path: string;
  dataUrl: string | null;
  createdAt: string;
}

export interface CollectionRecord {
  id: string;
  name: string;
  gameCount: number;
}

export interface CollectionMembership {
  collectionId: string;
  gameId: string;
}

export interface AutoScreenshotScanResult {
  found: number;
  imported: number;
  steamImported: number;
  matchedImported: number;
  skippedDuplicates: number;
}

export interface SaveConfig {
  profileId: string;
  gameId: string;
  savePath: string;
  configuredAt: string;
}

export interface SaveBackupRecord {
  id: string;
  profileId: string;
  gameId: string;
  backupPath: string;
  createdAt: string;
  fileCount: number;
  totalBytes: number;
  kind: "manual" | "pre-restore" | string;
}

export interface ProfileRecord {
  id: string;
  name: string;
  createdAt: string;
  lastUsedAt: string;
  gameCount: number;
  backupCount: number;
}

export interface ModProfileRecord {
  id: string;
  name: string;
  active: boolean;
  modCount: number;
  enabledCount: number;
}

export interface ModRecord {
  id: string;
  name: string;
  enabled: boolean;
  fileCount: number;
  deployedFileCount: number;
  totalBytes: number;
  source: string;
  sourceId: string | null;
  version: string | null;
  dependencies: string[];
}

export interface ThunderstoreModResult {
  namespace: string;
  name: string;
  version: string;
  description: string;
  icon: string | null;
  downloads: number;
  pinned: boolean;
  dependencyCount: number;
  packageUrl: string;
}

export interface ThunderstoreSearchResult {
  community: string;
  gameName: string;
  loader: string;
  mods: ThunderstoreModResult[];
}

export interface ModManagerOverview {
  gamePath: string;
  activeProfileId: string;
  profiles: ModProfileRecord[];
  mods: ModRecord[];
}

export interface ModToggleResult {
  modRecord: ModRecord;
  changedFiles: number;
  conflicts: string[];
}

export interface CloudSaveStatus {
  configured: boolean;
  authenticated: boolean;
  userId: string | null;
  message: string;
}

export interface CloudUploadResult {
  backupId: string;
  profileId: string;
  gameId: string;
  uploadedFiles: number;
  uploadedBytes: number;
}

export interface ProfileSaveFileState {
  profileId: string;
  gameId: string;
  vaultPath: string;
  exists: boolean;
  fileCount: number;
  totalBytes: number;
  updatedAt: string | null;
}

export interface CloudSyncAllResult {
  backups: number;
  uploadedFiles: number;
  uploadedBytes: number;
}
