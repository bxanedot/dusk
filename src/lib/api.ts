import { invoke } from "@tauri-apps/api/core";
import type {
  CollectionMembership,
  CollectionRecord,
  CheatTable,
  CloudUploadResult,
  CloudSyncAllResult,
  GameRecord,
  LaunchResult,
  ModManagerOverview,
  ModToggleResult,
  OnlineCheatTableResult,
  ProfileRecord,
  ProfileSaveFileState,
  PlatformAchievement,
  PlatformAchievementScanResult,
  ScanResult,
  SaveBackupRecord,
  SaveConfig,
  Stats,
  SteamAchievementStatus,
  TrainerCatalogResult,
  ThunderstoreModResult,
  ThunderstoreSearchResult,
} from "../types";

export type OnlineFixHosterFile = {
  provider: string;
  filename: string;
  url: string;
  isFix: boolean;
  directArchive: boolean;
  requiresCaution: boolean;
};

export type GameArchiveBundle = { label: string; parts: Array<{ filename: string; mirrors: string[] }> };

export type ManagedDownload = {
  id: string;
  title: string;
  filename: string;
  filePath: string;
  status: "downloading" | "completed" | "failed" | "cancelled";
  receivedBytes: number;
  totalBytes: number | null;
  error: string | null;
  bundleReady: boolean;
  listingUrl: string | null;
  source: "online-fix" | null;
};

export const api = {
  setAccountScope: (userId: string | null) => invoke<void>("set_account_scope", { accountUserId: userId }),
  exportAccountState: () => invoke<Record<string, unknown>>("export_account_state"),
  exportPlaytimeUpdates: () => invoke<{
    baselines: Array<{ game_id: string; total_seconds: number; launch_count: number; last_played: string | null }>;
    sessions: Array<{ device_id: string; session_id: number; game_id: string; duration_seconds: number; ended_at: string }>;
  }>("export_playtime_updates"),
  mergeCloudPlaytime: (totals: unknown[]) => invoke<void>("merge_cloud_playtime", { totals }),
  importAccountState: (state: Record<string, unknown>) => invoke<void>("import_account_state", { state }),
  listProfiles: () => invoke<ProfileRecord[]>("list_profiles"),
  getActiveProfile: () => invoke<ProfileRecord>("get_active_profile"),
  createProfile: (name: string) =>
    invoke<ProfileRecord>("create_profile", { name }),
  renameProfile: (profileId: string, name: string) =>
    invoke<ProfileRecord>("rename_profile", { profileId, name }),
  setActiveProfile: (profileId: string) =>
    invoke<ProfileRecord>("set_active_profile", { profileId }),
  deleteProfile: (profileId: string) =>
    invoke<ProfileRecord>("delete_profile", { profileId }),
  listModManager: (gameId: string) =>
    invoke<ModManagerOverview>("list_mod_manager", { gameId }),
  createModProfile: (gameId: string, name: string) =>
    invoke<ModManagerOverview>("create_mod_profile", { gameId, name }),
  setActiveModProfile: (gameId: string, profileId: string) =>
    invoke<ModManagerOverview>("set_active_mod_profile", { gameId, profileId }),
  importModFolder: (gameId: string) =>
    invoke<ModManagerOverview | null>("import_mod_folder", { gameId }),
  setModEnabled: (gameId: string, modId: string, enabled: boolean) =>
    invoke<ModToggleResult>("set_mod_enabled", { gameId, modId, enabled }),
  removeMod: (gameId: string, modId: string) =>
    invoke<ModManagerOverview>("remove_mod", { gameId, modId }),
  removeModProfile: (gameId: string, profileId: string) =>
    invoke<ModManagerOverview>("remove_mod_profile", { gameId, profileId }),
  searchThunderstoreMods: (gameId: string, query: string) =>
    invoke<ThunderstoreSearchResult>("search_thunderstore_mods", { gameId, query }),
  installThunderstoreMod: (gameId: string, mod: ThunderstoreModResult) =>
    invoke<ModManagerOverview>("install_thunderstore_mod", {
      gameId,
      namespace: mod.namespace,
      name: mod.name,
      version: mod.version,
    }),
  openModFolder: (modId: string) => invoke<void>("open_mod_folder", { modId }),
  listGames: () => invoke<GameRecord[]>("list_games"),
  listTrainerCatalog: () => invoke<TrainerCatalogResult[]>("list_trainer_catalog"),
  searchTrainerCatalog: (query: string) =>
    invoke<TrainerCatalogResult[]>("search_trainer_catalog", { query }),
  searchOnlineCheatTables: (query: string) =>
    invoke<OnlineCheatTableResult[]>("search_online_cheat_tables", { query }),
  getCheatTable: (gameId: string) => invoke<CheatTable | null>("get_cheat_table", { gameId }),
  importCheatTable: (gameId: string) => invoke<CheatTable | null>("import_cheat_table", { gameId }),
  importOnlineCheatTable: (gameId: string, result: OnlineCheatTableResult) =>
    invoke<CheatTable>("import_online_cheat_table", {
      gameId,
      repository: result.repository,
      blobSha: result.blobSha,
      fileName: result.fileName,
      sizeBytes: result.sizeBytes,
    }),
  applyCheatValue: (gameId: string, entryId: string, value: string) =>
    invoke<void>("apply_cheat_value", { gameId, entryId, value }),
  setCheatHold: (gameId: string, entryId: string, enabled: boolean, value?: string) =>
    invoke<void>("set_cheat_hold", { gameId, entryId, enabled, value }),
  scanGames: () => invoke<ScanResult>("scan_games"),
  refreshMissingCovers: () =>
    invoke<{ attempted: number; updated: number; remaining: number }>("refresh_missing_covers"),
  chooseExecutable: () => invoke<string | null>("choose_executable"),
  startArchiveBundle: (title: string, parts: GameArchiveBundle["parts"], listingUrl?: string, source?: "online-fix") =>
    invoke<ManagedDownload>("start_archive_bundle", {
      title, parts, listingUrl: listingUrl || null, source: source || null,
    }),
  startManagedDownload: (url: string, filename: string, title: string, source?: "online-fix") =>
    invoke<ManagedDownload>("start_managed_download", { url, filename, title, source: source || null }),
  listManagedDownloads: () => invoke<ManagedDownload[]>("list_managed_downloads"),
  cancelManagedDownload: (downloadId: string) =>
    invoke<void>("cancel_managed_download", { downloadId }),
  importGameArchive: () => invoke<{ directory: string; game: GameRecord | null; installers: string[] } | null>("import_game_archive"),
  verifyDownloadedMultipartArchive: (archivePath: string, password?: string) =>
    invoke<{ ready: boolean; message: string }>("verify_downloaded_multipart_archive", {
      archivePath, password: password || null,
    }),
  listRecentGameArchives: (sinceMs: number) =>
    invoke<Array<{ path: string; filename: string; sizeBytes: number; modifiedAtMs: number }>>("list_recent_game_archives", { sinceMs }),
  importDownloadedGameArchive: (archivePath: string, title: string, password?: string) =>
    invoke<{ directory: string; game: GameRecord | null; installers: string[] }>("import_downloaded_game_archive", {
      archivePath, title, password: password || null,
    }),
  chooseGameInstaller: () => invoke<string | null>("choose_game_installer"),
  runGameInstaller: (installerPath: string) =>
    invoke<void>("run_game_installer", { installerPath }),
  discoverGameSourceArchives: (source: "game3rb" | "fitgirl", listingUrl: string) =>
    invoke<GameArchiveBundle[]>("discover_game_source_archives", { source, listingUrl }),
  searchGameSource: (source: "game3rb" | "fitgirl", query: string) =>
    invoke<Array<{ title: string; url: string; description: string }>>(
      "search_game_source", { source, query },
    ),
  openGameSourceSearch: (source: "game3rb" | "fitgirl", query: string) =>
    invoke<void>("open_game_source_search", { source, query }),
  openGameSourceListing: (url: string) =>
    invoke<void>("open_game_source_listing", { url }),
  openGameSourceBrowser: (url: string) =>
    invoke<void>("open_game_source_browser", { url }),
  searchOnlineFixGames: (query: string) =>
    invoke<Array<{ title: string; url: string; description: string }>>("search_online_fix_games", { query }),
  getOnlineFixHosterFiles: (hostersUrl: string) =>
    invoke<OnlineFixHosterFile[]>("get_online_fix_hoster_files", { hostersUrl }),
  getOnlineFixDownloadLinks: (listingUrl: string) =>
    invoke<Array<{ url: string; label: string; kind: "game" | "mirror" | "torrent" | "fix"; recommended: boolean }>>(
      "get_online_fix_download_links", { listingUrl },
    ),
  openOnlineFixResult: (url: string, gameTitle?: string, autoSelect = false) =>
    invoke<void>("open_online_fix_result", {
      url, gameTitle: gameTitle || null, autoSelect,
    }),
  openOnlineFixBrowser: (url: string) =>
    invoke<void>("open_online_fix_browser", { url }),
  openExternalTarget: (target: "creator" | "site" | "steam-api-key" | "steam" | "epic" | "gog" | "itch") =>
    invoke<void>("open_external_target", { target }),
  openNexusModsSearch: (query: string) =>
    invoke<void>("open_nexus_mods_search", { query }),
  openCheatTableDirectorySearch: (query: string) =>
    invoke<void>("open_cheat_table_directory_search", { query }),
  openFearlessRevolutionSearch: (query: string) =>
    invoke<void>("open_fearless_revolution_search", { query }),
  addManualGame: (title: string, exePath: string) =>
    invoke<GameRecord>("add_manual_game", { title, exePath }),
  setFavorite: (gameId: string, favorite: boolean) =>
    invoke<void>("set_favorite", { gameId, favorite }),
  renameGame: (gameId: string, title: string) =>
    invoke<void>("rename_game", { gameId, title }),
  removeGame: (gameId: string) => invoke<void>("remove_game", { gameId }),
  chooseCover: (gameId: string) => invoke<boolean>("choose_cover", { gameId }),
  launchGame: (gameId: string) =>
    invoke<LaunchResult>("launch_game", { gameId }),
  openGameFolder: (gameId: string) =>
    invoke<void>("open_game_folder", { gameId }),
  listCollections: () => invoke<CollectionRecord[]>("list_collections"),
  createCollection: (name: string) =>
    invoke<CollectionRecord>("create_collection", { name }),
  deleteCollection: (collectionId: string) =>
    invoke<void>("delete_collection", { collectionId }),
  setCollectionMembership: (
    collectionId: string,
    gameId: string,
    included: boolean,
  ) =>
    invoke<void>("set_collection_membership", {
      collectionId,
      gameId,
      included,
    }),
  collectionMemberships: () =>
    invoke<CollectionMembership[]>("collection_memberships"),
  getStats: () => invoke<Stats>("get_stats"),
  listPlatformAchievements: () => invoke<PlatformAchievement[]>("list_platform_achievements"),
  steamAchievementStatus: () => invoke<SteamAchievementStatus>("steam_achievement_status"),
  saveSteamWebApiKey: (key: string) => invoke<void>("save_steam_web_api_key", { key }),
  removeSteamWebApiKey: () => invoke<void>("remove_steam_web_api_key"),
  scanPlatformAchievements: (force = false) =>
    invoke<PlatformAchievementScanResult>("scan_platform_achievements", { force }),
  chooseSaveFolder: (gameId: string) =>
    invoke<SaveConfig | null>("choose_save_folder", { gameId }),
  detectSaveFolder: (gameId: string) =>
    invoke<SaveConfig | null>("detect_save_folder", { gameId }),
  getProfileSaveFileState: (gameId: string) =>
    invoke<ProfileSaveFileState>("get_profile_save_file_state", { gameId }),
  saveProfileFiles: (gameId: string) =>
    invoke<ProfileSaveFileState>("save_profile_files", { gameId }),
  loadProfileFiles: (gameId: string) =>
    invoke<ProfileSaveFileState>("load_profile_files", { gameId }),
  getSaveConfig: (gameId: string) =>
    invoke<SaveConfig | null>("get_save_config", { gameId }),
  openSaveFolder: (gameId: string) =>
    invoke<void>("open_save_folder", { gameId }),
  clearSaveConfig: (gameId: string) =>
    invoke<void>("clear_save_config", { gameId }),
  createSaveBackup: (gameId: string) =>
    invoke<SaveBackupRecord>("create_save_backup", { gameId }),
  listSaveBackups: (gameId: string) =>
    invoke<SaveBackupRecord[]>("list_save_backups", { gameId }),
  restoreSaveBackup: (gameId: string, backupId: string) =>
    invoke<SaveBackupRecord>("restore_save_backup", { gameId, backupId }),
  deleteSaveBackup: (gameId: string, backupId: string) =>
    invoke<void>("delete_save_backup", { gameId, backupId }),
  uploadSaveBackupToCloud: (input: {
    backupId: string;
    supabaseUrl: string;
    publishableKey: string;
    accessToken: string;
    authUserId: string;
  }) => invoke<CloudUploadResult>("upload_save_backup_to_cloud", input),
  uploadAllSaveBackupsToCloud: (input: {
    supabaseUrl: string;
    publishableKey: string;
    accessToken: string;
    authUserId: string;
  }) => invoke<CloudSyncAllResult>("upload_all_save_backups_to_cloud", input),
  uploadCloudManifest: (input: {
    supabaseUrl: string;
    publishableKey: string;
    accessToken: string;
    authUserId: string;
  }) => invoke<void>("upload_cloud_manifest", input),
  discordRpcEnable: (clientId: string, state?: string) =>
    invoke<void>("discord_rpc_enable", { clientId, state: state ?? null }),
  discordRpcUpdate: (state: string) => invoke<void>("discord_rpc_update", { state }),
  discordRpcDisable: () => invoke<void>("discord_rpc_disable"),
  dataDirectory: () => invoke<string>("data_directory"),
};
