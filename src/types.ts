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

export interface ScanResult {
  found: number;
  added: number;
  updated: number;
  steamFound: number;
  epicFound: number;
  gogFound: number;
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
  screenshotCount: number;
  topGame: string | null;
}

export interface Achievement {
  id: string;
  title: string;
  description: string;
  unlocked: boolean;
  current: number;
  target: number;
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
