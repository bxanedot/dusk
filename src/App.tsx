import {
  Archive,
  Check,
  Cloud,
  Clock3,
  Database,
  ExternalLink,
  FolderOpen,
  Gamepad2,
  HardDrive,
  Globe2,
  Heart,
  Home,
  ImagePlus,
  Images,
  Layers3,
  Library,
  Minus,
  Moon,
  PackageOpen,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Radio,
  RotateCcw,
  ScanSearch,
  Save,
  ShieldCheck,
  Search,
  Settings,
  Sparkles,
  Square,
  Star,
  Trash2,
  Trophy,
  UserRound,
  X,
} from "lucide-react";
import {
  type FormEvent,
  type MouseEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import duskLogo from "./assets/dusk-logo.svg";
import AccountProfileSettings from "./AccountProfileSettings";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  TrainerCatalogBrowser,
  filterTrainerCatalogResults,
  normalizeCatalogText,
  type TrainerCatalogBrowserState,
  type TrainerCatalogStatusFilter,
} from "./components/TrainerCatalogBrowser";
import { TrainerCatalogPanel, type TrainerCatalogPanelState } from "./components/TrainerCatalogPanel";
import { TrainerPanel } from "./components/TrainerPanel";
import { onlineCheatTableKey, type OnlineCheatTableSearchState } from "./components/OnlineCheatTableResults";
import { api, type ManagedDownload, type OnlineFixHosterFile } from "./lib/api";
import { syncAccountState } from "./lib/accountSync";
import { currentDuskAccount, logoutDuskAccount, type DuskAccount } from "./lib/auth";
import { cloudSaveStatus, syncAllBackupsToSupabase, syncBackupToSupabase, syncCloudManifest } from "./lib/cloudSaves";
import { checkForDuskUpdate, installDuskUpdate } from "./lib/updater";
import { useControllerNavigation } from "./lib/useControllerNavigation";
import type {
  Achievement,
  CollectionMembership,
  CollectionRecord,
  CloudSaveStatus,
  GameRecord,
  OnlineCheatTableResult,
  ProfileRecord,
  ProfileSaveFileState,
  ScreenshotRecord,
  SaveBackupRecord,
  SaveConfig,
  Stats,
  TrainerCatalogResult,
} from "./types";

type View =
  | "home"
  | "library"
  | "favorites"
  | "trainerCatalog"
  | "screenshots"
  | "achievements"
  | "settings";

type SortMode = "name" | "recent" | "playtime";
type ThemeName = "night" | "oled" | "slate";
type AccentName = "violet" | "ember" | "cyan";
type WebGameSource = "online-fix" | "game3rb" | "fitgirl";
const WEB_SOURCES: Record<WebGameSource, { label: string; host: string; description: string }> = {
  "online-fix": { label: "Online-Fix", host: "online-fix.me", description: "Online game listings" },
  game3rb: { label: "Game3rb", host: "game3rb.com", description: "Game3rb game listings" },
  fitgirl: { label: "FitGirl · Offline", host: "fitgirl-repacks.site", description: "Offline game repack listings" },
};
type WebGameResult = { title: string; url: string; description: string };
type WebDownloadLink = {
  url: string;
  label: string;
  kind: "game" | "mirror" | "torrent" | "fix";
  recommended: boolean;
};

function matchesGameArchive(filename: string, title: string) {
  const clean = (text: string) => text.toLocaleLowerCase()
    .replace(/\b(online|multiplayer|co-op|coop)\b/g, " ")
    .replace(/по\s+сети/gu, " ")
    .replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  const tokens = clean(title).split(" ").filter((part) => part.length >= 2);
  const file = clean(filename.replace(/\.(zip|rar|7z)(\.\d+)?$/i, ""));
  const fileWords = file.split(" ");
  // Manually requested fix/repair/update archives are valid downloads, but
  // they are not complete games and should never be auto-registered.
  if (fileWords.some((word) => ["fix", "repair", "update", "updates", "patch", "crack", "redist"].includes(word))) return false;
  return tokens.length > 0 && tokens.every((token) => fileWords.includes(token));
}

const EMPTY_STATS: Stats = {
  gameCount: 0,
  favoriteCount: 0,
  playedGameCount: 0,
  totalSeconds: 0,
  launchCount: 0,
  last7DaysSeconds: 0,
  screenshotCount: 0,
  topGame: null,
};

function cx(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(" ");
}

function scrollToSettingsSection(sectionId: string) {
  document.getElementById(sectionId)?.scrollIntoView({ behavior: "smooth", block: "start" });
}

function readableError(error: unknown) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  try {
    return JSON.stringify(error);
  } catch {
    return "Unknown error";
  }
}

function formatDuration(totalSeconds: number) {
  if (!totalSeconds) return "0m";
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  if (hours >= 100) return String(hours) + "h";
  if (hours > 0) return String(hours) + "h " + String(minutes) + "m";
  return String(Math.max(1, minutes)) + "m";
}

function formatBytes(bytes: number) {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return (unit === 0 ? String(Math.round(value)) : value.toFixed(value >= 10 ? 1 : 2)) + " " + units[unit];
}

function formatDate(value: string | null) {
  if (!value) return "Never";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Unknown";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function sourceLabel(source: string) {
  if (source === "steam") return "Steam";
  if (source === "epic") return "Epic";
  if (source === "gog") return "GOG";
  if (source === "emulator") return "Emulator";
  if (source === "device") return "Device";
  if (source === "manual") return "Manual";
  return source;
}

function NavItem(props: {
  active: boolean;
  icon: ReactNode;
  label: string;
  count?: number;
  onClick: () => void;
}) {
  return (
    <button className={cx("nav-item", props.active && "active")} onClick={props.onClick}>
      <span className="nav-icon">{props.icon}</span>
      <span>{props.label}</span>
      {typeof props.count === "number" && <span className="nav-count">{props.count}</span>}
    </button>
  );
}

function Cover({ game }: { game: GameRecord }) {
  if (game.coverPath) {
    return (
      <div className="cover">
        <img
          src={convertFileSrc(game.coverPath)}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
        />
      </div>
    );
  }

  const initials = game.title
    .split(" ")
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part.slice(0, 1).toUpperCase())
    .join("");

  return (
    <div className="cover cover-placeholder">
      <span>{initials || "D"}</span>
      <small>{sourceLabel(game.source)}</small>
    </div>
  );
}

function GameCard(props: {
  game: GameRecord;
  onOpen: () => void;
  onPlay: () => void;
  onFavorite: () => void;
  playing: boolean;
}) {
  function play(event: MouseEvent<HTMLButtonElement>) {
    event.stopPropagation();
    props.onPlay();
  }

  function favorite(event: MouseEvent<HTMLButtonElement>) {
    event.stopPropagation();
    props.onFavorite();
  }

  return (
    <article
      className="game-card"
      data-controller-game-id={props.game.id}
      role="button"
      tabIndex={0}
      onClick={props.onOpen}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") props.onOpen();
      }}
    >
      <div className="game-art">
        <Cover game={props.game} />
        <div className="game-overlay">
          <button
            className="round-action primary"
            onClick={play}
            aria-label={"Play " + props.game.title}
            disabled={props.playing}
          >
            {props.playing ? <RefreshCw className="spin" size={18} /> : <Play size={18} />}
          </button>
          <button
            className={cx("round-action", props.game.favorite && "selected")}
            onClick={favorite}
            aria-label={props.game.favorite ? "Remove favorite" : "Add favorite"}
          >
            <Star size={17} fill={props.game.favorite ? "currentColor" : "none"} />
          </button>
        </div>
      </div>
      <div className="game-card-copy">
        <strong title={props.game.title}>{props.game.title}</strong>
        <div className="game-meta">
          <span>{sourceLabel(props.game.source)}</span>
          <span>•</span>
          <span>{formatDuration(props.game.totalSeconds)}</span>
        </div>
      </div>
    </article>
  );
}

function EmptyState(props: {
  icon: ReactNode;
  title: string;
  copy: string;
  action?: ReactNode;
}) {
  return (
    <div className="empty-state">
      <div className="empty-icon">{props.icon}</div>
      <h3>{props.title}</h3>
      <p>{props.copy}</p>
      {props.action}
    </div>
  );
}

function AddGameModal(props: {
  onClose: () => void;
  onAdded: () => Promise<void>;
}) {
  const [title, setTitle] = useState("");
  const [path, setPath] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const savingRef = useRef(false);
  const browsingRef = useRef(false);

  async function browse() {
    if (browsingRef.current || savingRef.current) return;
    browsingRef.current = true;
    try {
      const picked = await api.chooseExecutable();
      if (picked) setPath(picked);
    } catch (err) {
      setError(readableError(err));
    } finally {
      browsingRef.current = false;
    }
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (savingRef.current) return;
    setError("");
    if (!title.trim() || !path.trim()) {
      setError("Choose an executable and enter a game title.");
      return;
    }

    savingRef.current = true;
    setSaving(true);
    try {
      await api.addManualGame(title.trim(), path.trim());
      await props.onAdded();
      props.onClose();
    } catch (err) {
      setError(readableError(err));
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  }

  return (
    <div className="modal-backdrop" onMouseDown={props.onClose}>
      <form
        className="modal"
        onSubmit={submit}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="modal-head">
          <div>
            <span className="eyebrow">Manual library entry</span>
            <h2>Add game</h2>
          </div>
          <button type="button" className="icon-button" onClick={props.onClose}>
            <X size={18} />
          </button>
        </div>

        <label>
          <span>Game title</span>
          <input
            autoFocus
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="Game name"
          />
        </label>

        <label>
          <span>Executable</span>
          <div className="path-picker">
            <input value={path} readOnly placeholder="Choose a .exe file" />
            <button type="button" className="button secondary" onClick={browse}>
              Browse
            </button>
          </div>
        </label>

        {error && <div className="inline-error">{error}</div>}

        <div className="modal-actions">
          <button type="button" className="button ghost" onClick={props.onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={saving}>
            {saving ? "Adding…" : "Add to Dusk"}
          </button>
        </div>
      </form>
    </div>
  );
}

function GameDetail(props: {
  game: GameRecord;
  activeProfile: ProfileRecord | null;
  collections: CollectionRecord[];
  memberships: CollectionMembership[];
  onClose: () => void;
  onPlay: (game: GameRecord) => Promise<void>;
  onRefresh: () => Promise<void>;
  onToast: (message: string, type?: "ok" | "error") => void;
  onSearchCatalog: (query: string) => Promise<TrainerCatalogResult[]>;
  onBrowseCatalog: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const detailBusyRef = useRef(false);
  const [saveConfig, setSaveConfig] = useState<SaveConfig | null>(null);
  const [saveBackups, setSaveBackups] = useState<SaveBackupRecord[]>([]);
  const [profileSaveState, setProfileSaveState] = useState<ProfileSaveFileState | null>(null);
  const [saveLoading, setSaveLoading] = useState(true);
  const [trainerCatalogState, setTrainerCatalogState] = useState<TrainerCatalogPanelState>("idle");
  const [trainerCatalogResults, setTrainerCatalogResults] = useState<TrainerCatalogResult[]>([]);
  const [trainerCatalogError, setTrainerCatalogError] = useState<string | null>(null);
  const trainerCatalogRequest = useRef(0);

  const membershipSet = useMemo(() => {
    return new Set(
      props.memberships
        .filter((item) => item.gameId === props.game.id)
        .map((item) => item.collectionId),
    );
  }, [props.memberships, props.game.id]);

  async function refreshSaveData() {
    setSaveLoading(true);
    try {
      const values = await Promise.all([
        api.getSaveConfig(props.game.id),
        api.listSaveBackups(props.game.id),
        api.getProfileSaveFileState(props.game.id),
      ]);
      setSaveConfig(values[0]);
      setSaveBackups(values[1]);
      setProfileSaveState(values[2]);
    } catch (error) {
      props.onToast(readableError(error), "error");
    } finally {
      setSaveLoading(false);
    }
  }

  useEffect(() => {
    void refreshSaveData();
  }, [props.game.id]);

  useEffect(() => {
    trainerCatalogRequest.current += 1;
    setTrainerCatalogState("idle");
    setTrainerCatalogResults([]);
    setTrainerCatalogError(null);
    return () => {
      trainerCatalogRequest.current += 1;
    };
  }, [props.game.id]);

  async function searchTrainerCatalog() {
    const requestId = ++trainerCatalogRequest.current;
    setTrainerCatalogState("loading");
    setTrainerCatalogResults([]);
    setTrainerCatalogError(null);
    try {
      const results = await props.onSearchCatalog(props.game.title);
      if (trainerCatalogRequest.current !== requestId) return;
      setTrainerCatalogResults(results);
      setTrainerCatalogState(results.length === 0 ? "empty" : "results");
    } catch (error) {
      if (trainerCatalogRequest.current !== requestId) return;
      setTrainerCatalogError(readableError(error) || "Could not load trainer results. Try again.");
      setTrainerCatalogState("error");
    }
  }

  async function withDetailLock<T>(action: () => Promise<T>): Promise<T | undefined> {
    if (detailBusyRef.current) return undefined;
    detailBusyRef.current = true;
    setBusy(true);
    try {
      return await action();
    } finally {
      detailBusyRef.current = false;
      setBusy(false);
    }
  }

  async function configureSaveFolder() {
    await withDetailLock(async () => {
      try {
        const config = await api.chooseSaveFolder(props.game.id);
        if (config) {
          setSaveConfig(config);
          props.onToast("Save folder configured.");
          await refreshSaveData();
        }
      } catch (error) {
        props.onToast(readableError(error), "error");
      }
    });
  }

  async function saveFilesToProfile() {
    await withDetailLock(async () => {
      try {
        const state = await api.saveProfileFiles(props.game.id);
        setProfileSaveState(state);
        props.onToast(
          "Saved " +
            String(state.fileCount) +
            " files to " +
            (props.activeProfile?.name || "this profile") +
            ".",
        );
      } catch (error) {
        props.onToast(readableError(error), "error");
      }
    });
  }

  async function loadFilesFromProfile() {
    if (detailBusyRef.current) return;
    const okay = window.confirm(
      "Load " +
        (props.activeProfile?.name || "this profile") +
        "'s saved files for " +
        props.game.title +
        "? Dusk creates a safety backup before replacing the live save folder.",
    );
    if (!okay) return;

    await withDetailLock(async () => {
      try {
        const state = await api.loadProfileFiles(props.game.id);
        setProfileSaveState(state);
        props.onToast(
          "Loaded " +
            String(state.fileCount) +
            " profile save files. A safety backup was created first.",
        );
        await refreshSaveData();
      } catch (error) {
        props.onToast(readableError(error), "error");
      }
    });
  }

  async function backupSaves() {
    await withDetailLock(async () => {
      try {
        const backup = await api.createSaveBackup(props.game.id);
        let cloudSuffix = "";
        try {
          const status = await cloudSaveStatus();
          if (status.configured && status.authenticated) {
            const uploaded = await syncBackupToSupabase(backup);
            await syncCloudManifest();
            cloudSuffix =
              " · Supabase: " +
              String(uploaded.uploadedFiles) +
              " files synced";
          } else if (status.configured) {
            cloudSuffix = " · cloud sync unavailable";
          }
        } catch {
          cloudSuffix = " · saved locally; cloud sync will retry later";
        }
        props.onToast(
          "Save backup created: " +
            String(backup.fileCount) +
            " files · " +
            formatBytes(backup.totalBytes) +
            cloudSuffix +
            ".",
        );
        await refreshSaveData();
      } catch (error) {
        props.onToast(readableError(error), "error");
      }
    });
  }

  async function restoreBackup(backup: SaveBackupRecord) {
    if (detailBusyRef.current) return;
    const okay = window.confirm(
      "Restore this save backup from " +
        formatDate(backup.createdAt) +
        "? Dusk will create a safety backup of your current saves first.",
    );
    if (!okay) return;

    await withDetailLock(async () => {
      try {
        const safety = await api.restoreSaveBackup(props.game.id, backup.id);
        try {
          const status = await cloudSaveStatus();
          if (status.configured && status.authenticated) {
            await syncBackupToSupabase(safety);
            await syncCloudManifest();
          }
        } catch {
          // A local safety backup is still valid if cloud sync is unavailable.
        }
        props.onToast(
          "Save restored. Safety backup created with " +
            String(safety.fileCount) +
            " files.",
        );
        await refreshSaveData();
      } catch (error) {
        props.onToast(readableError(error), "error");
      }
    });
  }

  async function deleteBackup(backup: SaveBackupRecord) {
    if (detailBusyRef.current) return;
    const okay = window.confirm(
      "Delete this Dusk backup? Your current live save files will not be changed.",
    );
    if (!okay) return;

    await withDetailLock(async () => {
      try {
        await api.deleteSaveBackup(props.game.id, backup.id);
        props.onToast("Save backup deleted.");
        await refreshSaveData();
      } catch (error) {
        props.onToast(readableError(error), "error");
      }
    });
  }

  async function clearSaveFolder() {
    if (detailBusyRef.current) return;
    const okay = window.confirm(
      "Stop managing this save folder? Existing Dusk backups will be kept.",
    );
    if (!okay) return;

    await withDetailLock(async () => {
      try {
        await api.clearSaveConfig(props.game.id);
        props.onToast("Save folder disconnected.");
        await refreshSaveData();
      } catch (error) {
        props.onToast(readableError(error), "error");
      }
    });
  }

  async function run(action: () => Promise<unknown>, success?: string) {
    await withDetailLock(async () => {
      try {
        await action();
        if (success) props.onToast(success);
        await props.onRefresh();
      } catch (error) {
        props.onToast(readableError(error), "error");
      }
    });
  }

  async function rename() {
    const title = window.prompt("Rename game", props.game.title);
    if (!title || !title.trim() || title.trim() === props.game.title) return;
    await run(() => api.renameGame(props.game.id, title.trim()), "Game renamed.");
  }

  async function remove() {
    const okay = window.confirm(
      'Remove "' + props.game.title + '" from Dusk? The game files will not be deleted.',
    );
    if (!okay) return;
    await run(() => api.removeGame(props.game.id), "Removed from Dusk.");
    props.onClose();
  }

  return (
    <div className="detail-backdrop" onMouseDown={props.onClose}>
      <aside className="detail-panel" onMouseDown={(event) => event.stopPropagation()}>
        <div className="detail-art">
          <Cover game={props.game} />
          <button className="icon-button detail-close" onClick={props.onClose}>
            <X size={19} />
          </button>
        </div>

        <div className="detail-body">
          <span className="source-pill">{sourceLabel(props.game.source)}</span>
          <h2>{props.game.title}</h2>
          <p className="detail-subtitle">
            {formatDuration(props.game.totalSeconds)} played · {props.game.launchCount} tracked{" "}
            {props.game.launchCount === 1 ? "session" : "sessions"}
          </p>

          <button className="button primary wide" onClick={() => props.onPlay(props.game)}>
            <Play size={17} fill="currentColor" />
            Play
          </button>

          <div className="detail-actions">
            <button
              className="detail-action"
              disabled={busy}
              onClick={() =>
                run(
                  () => api.setFavorite(props.game.id, !props.game.favorite),
                  props.game.favorite ? "Removed from favorites." : "Added to favorites.",
                )
              }
            >
              <Heart size={17} fill={props.game.favorite ? "currentColor" : "none"} />
              {props.game.favorite ? "Favorited" : "Favorite"}
            </button>
            <button
              className="detail-action"
              disabled={busy}
              onClick={() => run(() => api.openGameFolder(props.game.id))}
            >
              <FolderOpen size={17} />
              Folder
            </button>
            <button
              className="detail-action"
              disabled={busy}
              onClick={() =>
                run(async () => {
                  const changed = await api.chooseCover(props.game.id);
                  if (changed) props.onToast("Cover updated.");
                })
              }
            >
              <ImagePlus size={17} />
              Cover
            </button>
            <button
              className="detail-action"
              disabled={busy}
              onClick={() =>
                run(async () => {
                  const count = await api.importScreenshots(props.game.id);
                  props.onToast(
                    count === 0
                      ? "No screenshots imported."
                      : "Imported " + String(count) + " screenshot" + (count === 1 ? "." : "s."),
                  );
                })
              }
            >
              <Images size={17} />
              Shots
            </button>
          </div>

          <TrainerCatalogPanel
            gameTitle={props.game.title}
            state={trainerCatalogState}
            results={trainerCatalogResults}
            errorMessage={trainerCatalogError}
            onSearch={() => void searchTrainerCatalog()}
            onBrowse={props.onBrowseCatalog}
          />

          <TrainerPanel
            gameId={props.game.id}
            gameTitle={props.game.title}
            hasExecutable={Boolean(props.game.exePath)}
            onToast={props.onToast}
          />

          <section className="detail-section">
            <h3>Collections</h3>
            {props.collections.length === 0 ? (
              <p className="muted">Create a collection from the sidebar first.</p>
            ) : (
              <div className="collection-checks">
                {props.collections.map((collection) => {
                  const included = membershipSet.has(collection.id);
                  return (
                    <button
                      key={collection.id}
                      disabled={busy}
                      className={cx("collection-check", included && "included")}
                      onClick={() =>
                        run(() =>
                          api.setCollectionMembership(
                            collection.id,
                            props.game.id,
                            !included,
                          ),
                        )
                      }
                    >
                      <span>{collection.name}</span>
                      {included && <Check size={16} />}
                    </button>
                  );
                })}
              </div>
            )}
          </section>

          <section className="detail-section">
            <div className="detail-section-heading">
              <h3>Save backups</h3>
              {saveConfig && (
                <button className="text-action" disabled={busy} onClick={() => void backupSaves()}>
                  <Save size={14} />
                  Back up now
                </button>
              )}
            </div>

            {saveLoading ? (
              <p className="muted">Loading save backup settings…</p>
            ) : !saveConfig ? (
              <div className="save-empty">
                <Archive size={18} />
                <div>
                  <strong>No save folder configured</strong>
                  <p>
                    Select the exact folder this game uses for saves. Dusk will not guess or
                    modify save locations automatically.
                  </p>
                </div>
                <button
                  className="button secondary"
                  disabled={busy}
                  onClick={() => void configureSaveFolder()}
                >
                  Choose folder
                </button>
              </div>
            ) : (
              <div className="save-manager">
                <div className="save-config-row">
                  <div>
                    <span>Managed save folder</span>
                    <code title={saveConfig.savePath}>{saveConfig.savePath}</code>
                  </div>
                  <div className="save-config-actions">
                    <button
                      className="text-action"
                      disabled={busy}
                      onClick={() => void configureSaveFolder()}
                    >
                      <FolderOpen size={14} />
                      Change
                    </button>
                    <button
                      className="text-action danger"
                      disabled={busy}
                      onClick={() => void clearSaveFolder()}
                    >
                      <X size={14} />
                      Disconnect
                    </button>
                  </div>
                </div>

                <div className="profile-save-vault">
                  <div className="profile-save-vault-copy">
                    <strong>
                      {props.activeProfile?.name || "Owner"} · Profile save files
                    </strong>
                    <span>
                      {profileSaveState?.exists
                        ? String(profileSaveState.fileCount) +
                          " files · " +
                          formatBytes(profileSaveState.totalBytes) +
                          (profileSaveState.updatedAt
                            ? " · " + formatDate(profileSaveState.updatedAt)
                            : "")
                        : "No physical save files stored for this owner yet."}
                    </span>
                  </div>
                  <div className="profile-save-vault-actions">
                    <button
                      className="button secondary"
                      disabled={busy}
                      onClick={() => void saveFilesToProfile()}
                    >
                      <Save size={14} />
                      Save to profile
                    </button>
                    <button
                      className="button secondary"
                      disabled={busy || !profileSaveState?.exists}
                      onClick={() => void loadFilesFromProfile()}
                    >
                      <RotateCcw size={14} />
                      Load profile files
                    </button>
                  </div>
                </div>

                {saveBackups.length === 0 ? (
                  <p className="muted">
                    No restore points yet. Create one before changing mods, settings, or game files.
                  </p>
                ) : (
                  <div className="backup-list">
                    {saveBackups.map((backup) => (
                      <div className="backup-row" key={backup.id}>
                        <div className="backup-copy">
                          <strong>
                            {backup.kind === "pre-restore" ? "Safety backup" : "Manual backup"}
                          </strong>
                          <span>
                            {formatDate(backup.createdAt)} · {backup.fileCount} files ·{" "}
                            {formatBytes(backup.totalBytes)}
                          </span>
                        </div>
                        <div className="backup-actions">
                          <button
                            className="icon-button"
                            disabled={busy}
                            title="Restore this backup"
                            onClick={() => void restoreBackup(backup)}
                          >
                            <RotateCcw size={15} />
                          </button>
                          <button
                            className="icon-button"
                            disabled={busy}
                            title="Delete this backup"
                            onClick={() => void deleteBackup(backup)}
                          >
                            <Trash2 size={15} />
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
          </section>

          <section className="detail-section">
            <h3>Details</h3>
            <dl className="detail-list">
              <div>
                <dt>Last played</dt>
                <dd>{formatDate(props.game.lastPlayed)}</dd>
              </div>
              <div>
                <dt>Installed at</dt>
                <dd title={props.game.installPath}>{props.game.installPath}</dd>
              </div>
              <div>
                <dt>Executable</dt>
                <dd title={props.game.exePath || ""}>
                  {props.game.exePath || "Not detected"}
                </dd>
              </div>
            </dl>
          </section>

          <div className="detail-footer">
            <button className="text-action" onClick={rename} disabled={busy}>
              <Pencil size={15} />
              Rename
            </button>
            <button className="text-action danger" onClick={remove} disabled={busy}>
              <Trash2 size={15} />
              Remove from Dusk
            </button>
          </div>
        </div>
      </aside>
    </div>
  );
}

export default function App() {
  const [appVersion, setAppVersion] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    void getVersion()
      .then((version) => { if (!cancelled) setAppVersion(version); })
      .catch(() => { if (!cancelled) setAppVersion(null); });
    return () => { cancelled = true; };
  }, []);

  const [view, setView] = useState<View>("home");
  const [trainerCatalogState, setTrainerCatalogState] = useState<TrainerCatalogBrowserState>("idle");
  const [trainerCatalogEntries, setTrainerCatalogEntries] = useState<TrainerCatalogResult[]>([]);
  const [trainerCatalogError, setTrainerCatalogError] = useState<string | null>(null);
  const [trainerCatalogQuery, setTrainerCatalogQuery] = useState("");
  const [trainerCatalogStatusFilter, setTrainerCatalogStatusFilter] = useState<TrainerCatalogStatusFilter>("all");
  const [trainerCatalogPage, setTrainerCatalogPage] = useState(1);
  const [catalogOnlineTableState, setCatalogOnlineTableState] = useState<OnlineCheatTableSearchState>("idle");
  const [catalogOnlineTableResults, setCatalogOnlineTableResults] = useState<OnlineCheatTableResult[]>([]);
  const [catalogOnlineTableError, setCatalogOnlineTableError] = useState<string | null>(null);
  const [catalogOnlineTableBusyId, setCatalogOnlineTableBusyId] = useState<string | null>(null);
  const catalogOnlineTableRequest = useRef(0);
  const trainerCatalogEntriesRef = useRef<TrainerCatalogResult[] | null>(null);
  const trainerCatalogLoadRef = useRef<Promise<TrainerCatalogResult[]> | null>(null);
  const [games, setGames] = useState<GameRecord[]>([]);
  const [stats, setStats] = useState<Stats>(EMPTY_STATS);
  const [achievements, setAchievements] = useState<Achievement[]>([]);
  const [collections, setCollections] = useState<CollectionRecord[]>([]);
  const [memberships, setMemberships] = useState<CollectionMembership[]>([]);
  const [screenshots, setScreenshots] = useState<ScreenshotRecord[]>([]);
  const [dataDirectory, setDataDirectory] = useState("");
  const [profiles, setProfiles] = useState<ProfileRecord[]>([]);
  const [activeProfile, setActiveProfile] = useState<ProfileRecord | null>(null);
  const [accountIdentity, setAccountIdentity] = useState<DuskAccount | null>(null);
  const [cloudStatus, setCloudStatus] = useState<CloudSaveStatus>({
    configured: false,
    authenticated: false,
    userId: null,
    message: "Checking Supabase cloud saves…",
  });

  const [selectedGameId, setSelectedGameId] = useState<string | null>(null);
  const [collectionFilter, setCollectionFilter] = useState("all");
  const [query, setQuery] = useState("");
  const [webSearchEnabled, setWebSearchEnabled] = useState(false);
  const [webSource, setWebSource] = useState<WebGameSource>("online-fix");
  const [webResults, setWebResults] = useState<WebGameResult[]>([]);
  const [webSearched, setWebSearched] = useState(false);
  const [webLoading, setWebLoading] = useState(false);
  const [webError, setWebError] = useState("");
  const [downloadSources, setDownloadSources] = useState<Record<string, WebDownloadLink[]>>({});
  const [hosterFiles, setHosterFiles] = useState<Record<string, OnlineFixHosterFile[]>>({});
  const [downloadSourcesBusy, setDownloadSourcesBusy] = useState<string | null>(null);
  const [managedDownloads, setManagedDownloads] = useState<ManagedDownload[]>([]);
  // Transfers and extraction continue independently of the current screen.
  // No Downloads dashboard or manual URL form occupies Home/Library.
  const handledDownloadIdsRef = useRef(new Set<string>());
  const failedDownloadIdsRef = useRef(new Set<string>());
  const importingInBackgroundRef = useRef(false);
  const managedDownloadPathsRef = useRef(new Set<string>());
  const [activeDownloadWatch, setActiveDownloadWatch] = useState<{ sinceMs: number; title: string; url: string; source: WebGameSource } | null>(null);
  const [downloadWatchMessage, setDownloadWatchMessage] = useState("");
  const downloadStabilityRef = useRef(new Map<string, { size: number; stable: number }>());
  const watchBusyRef = useRef(false);
  const watchAttemptedRef = useRef(new Set<string>());
  const watchVerificationCheckedRef = useRef(new Map<string, number>());
  const watchImportRetryAtRef = useRef(new Map<string, number>());
  const [sourceFilter, setSourceFilter] = useState("all");
  const [sortMode, setSortMode] = useState<SortMode>("name");
  const [addOpen, setAddOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [screenshotScanning, setScreenshotScanning] = useState(false);
  const [playingId, setPlayingId] = useState<string | null>(null);
  const [toast, setToast] = useState<{ message: string; type: "ok" | "error" } | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [installerBusy, setInstallerBusy] = useState(false);
  const [archiveBusy, setArchiveBusy] = useState(false);
  const [cloudSyncBusy, setCloudSyncBusy] = useState(false);
  const [availableUpdate, setAvailableUpdate] = useState<string | null>(null);
  const [availableUpdateNotes, setAvailableUpdateNotes] = useState<string | null>(null);
  const [autoScanEnabled, setAutoScanEnabled] = useState(
    () => localStorage.getItem("dusk-auto-scan") !== "false",
  );
  const [lastAutoScanAt, setLastAutoScanAt] = useState<number | null>(() => {
    const value = Number(localStorage.getItem("dusk-last-auto-scan") || "0");
    return Number.isFinite(value) && value > 0 ? value : null;
  });
  const [windowMaximized, setWindowMaximized] = useState(false);
  const scanLockRef = useRef(false);
  const autoScanTimerRef = useRef<number | null>(null);
  const screenshotScanLockRef = useRef(false);
  const playingIdsRef = useRef(new Set<string>());
  const favoriteLocksRef = useRef(new Set<string>());
  const collectionMutationRef = useRef(false);
  const screenshotDeleteLocksRef = useRef(new Set<number>());
  const updateLockRef = useRef(false);
  const installerLockRef = useRef(false);
  const consoleModeLockRef = useRef(false);
  const windowActionLockRef = useRef(false);
  const refreshCoreBusyRef = useRef(false);
  const refreshCorePendingRef = useRef(false);
  const refreshScreenshotsBusyRef = useRef(false);
  const refreshScreenshotsPendingRef = useRef(false);
  const toastTimerRef = useRef<number | null>(null);
  const screenshotsLoadedRef = useRef(false);
  const [consoleMode, setConsoleMode] = useState(
    () => localStorage.getItem("dusk-console-mode") === "true",
  );

  const [theme, setTheme] = useState<ThemeName>(
    () => (localStorage.getItem("dusk-theme") as ThemeName) || "night",
  );
  const [accent, setAccent] = useState<AccentName>(
    () => (localStorage.getItem("dusk-accent") as AccentName) || "violet",
  );
  const [discordRpcEnabled, setDiscordRpcEnabled] = useState(
    () => localStorage.getItem("dusk-discord-rpc-enabled") === "true",
  );
  const [discordClientId, setDiscordClientId] = useState(
    () => localStorage.getItem("dusk-discord-client-id") || "",
  );

  const showToast = useCallback((message: string, type: "ok" | "error" = "ok") => {
    if (toastTimerRef.current !== null) {
      window.clearTimeout(toastTimerRef.current);
    }
    setToast({ message, type });
    toastTimerRef.current = window.setTimeout(() => {
      setToast(null);
      toastTimerRef.current = null;
    }, 3600);
  }, []);

  function openCatalogGame(result: TrainerCatalogResult) {
    const catalogTitles = new Set([result.title, ...(result.aliases ?? [])].map(normalizeCatalogText));
    const matchingGames = games.filter((game) => catalogTitles.has(normalizeCatalogText(game.title)));
    const exactTitleMatch = matchingGames.find((game) => normalizeCatalogText(game.title) === normalizeCatalogText(result.title));
    const game = exactTitleMatch ?? (matchingGames.length === 1 ? matchingGames[0] : null);
    if (game) {
      setSelectedGameId(game.id);
      return;
    }
    if (matchingGames.length > 1) {
      showToast(`More than one library entry matches ${result.title}. Open it from Library to choose the right installation.`, "error");
      return;
    }
    showToast(`${result.title} is in Dusk Catalog. Add or scan it in your library, then select this card to open its trainer controls.`, "error");
  }

  async function loadTrainerCatalog(force = false): Promise<TrainerCatalogResult[]> {
    if (trainerCatalogEntriesRef.current && !force) {
      setTrainerCatalogEntries(trainerCatalogEntriesRef.current);
      setTrainerCatalogState("ready");
      setTrainerCatalogError(null);
      return trainerCatalogEntriesRef.current;
    }
    if (trainerCatalogLoadRef.current) return trainerCatalogLoadRef.current;

    setTrainerCatalogState("loading");
    setTrainerCatalogError(null);
    const request = api.listTrainerCatalog()
      .then((entries) => {
        trainerCatalogEntriesRef.current = entries;
        setTrainerCatalogEntries(entries);
        setTrainerCatalogState("ready");
        return entries;
      })
      .catch((error) => {
        setTrainerCatalogError(readableError(error) || "Could not load the catalog. Try again.");
        setTrainerCatalogState("error");
        throw error;
      })
      .finally(() => {
        if (trainerCatalogLoadRef.current === request) trainerCatalogLoadRef.current = null;
      });
    trainerCatalogLoadRef.current = request;
    return request;
  }

  async function searchTrainerCatalog(query: string): Promise<TrainerCatalogResult[]> {
    const matches = await api.searchTrainerCatalog(query);
    if (matches.length > 0) return matches;
    const entries = await loadTrainerCatalog();
    return filterTrainerCatalogResults(entries, query, "all").slice(0, 25);
  }

  async function searchCatalogOnlineTables(query: string) {
    const requestId = ++catalogOnlineTableRequest.current;
    setCatalogOnlineTableState("loading");
    setCatalogOnlineTableResults([]);
    setCatalogOnlineTableError(null);
    try {
      const results = await api.searchOnlineCheatTables(query);
      if (catalogOnlineTableRequest.current !== requestId) return;
      setCatalogOnlineTableResults(results);
      setCatalogOnlineTableState(results.length > 0 ? "results" : "empty");
    } catch (error) {
      if (catalogOnlineTableRequest.current !== requestId) return;
      setCatalogOnlineTableError(readableError(error) || "Could not search online tables.");
      setCatalogOnlineTableState("error");
    }
  }

  async function searchNexusMods(query: string) {
    try {
      await api.openNexusModsSearch(query);
    } catch (error) {
      showToast(readableError(error) || "Could not open Nexus Mods search.", "error");
    }
  }

  async function importCatalogOnlineTable(result: OnlineCheatTableResult) {
    const catalogMatch = filterTrainerCatalogResults(trainerCatalogEntries, result.gameTitle, "all")[0];
    const possibleTitles = new Set([
      result.gameTitle,
      ...(catalogMatch ? [catalogMatch.title, ...(catalogMatch.aliases ?? [])] : []),
    ].map(normalizeCatalogText));
    const matches = games.filter((game) => possibleTitles.has(normalizeCatalogText(game.title)));
    if (matches.length === 0) {
      showToast(`Add or scan ${catalogMatch?.title || result.gameTitle} in your Dusk library before importing its table.`, "error");
      return;
    }
    if (matches.length > 1) {
      showToast(`More than one library entry matches ${catalogMatch?.title || result.gameTitle}. Open it from Library to choose the right installation.`, "error");
      return;
    }

    const game = matches[0];
    try {
      const currentTable = await api.getCheatTable(game.id);
      if (currentTable && !window.confirm(`Replace the current table for ${game.title} with ${result.fileName}?`)) return;
      const key = onlineCheatTableKey(result);
      setCatalogOnlineTableBusyId(key);
      const imported = await api.importOnlineCheatTable(game.id, result);
      const skipped = imported.skippedEntries > 0
        ? ` ${imported.skippedEntries} unsupported entr${imported.skippedEntries === 1 ? "y was" : "ies were"} skipped.`
        : "";
      showToast(`Imported ${imported.entries.length} entries from ${result.fileName} for ${game.title}.${skipped}`);
      setSelectedGameId(game.id);
      setView("library");
    } catch (error) {
      showToast(readableError(error) || "Could not download or import that table.", "error");
    } finally {
      setCatalogOnlineTableBusyId(null);
    }
  }

  function discordPresenceState() {
    if (playingId) {
      const game = games.find((item) => item.id === playingId);
      return game ? "Launching " + game.title : "Launching a game";
    }
    if (view === "library" || view === "favorites") return "Browsing library";
    if (view === "screenshots") return "Viewing screenshots";
    if (view === "achievements") return "Viewing achievements";
    if (view === "settings") return "Adjusting Dusk settings";
    return "Using Dusk";
  }

  async function enableDiscordRpc() {
    const clientId = discordClientId.trim();
    if (!/^\d{15,24}$/.test(clientId)) {
      showToast("Enter a valid Discord Application ID first.", "error");
      return;
    }

    try {
      await api.discordRpcEnable(clientId, discordPresenceState());
      localStorage.setItem("dusk-discord-client-id", clientId);
      localStorage.setItem("dusk-discord-rpc-enabled", "true");
      setDiscordClientId(clientId);
      setDiscordRpcEnabled(true);
      showToast("Dusk Rich Presence connected to Discord.");
    } catch (error) {
      showToast(readableError(error), "error");
    }
  }

  async function disableDiscordRpc() {
    localStorage.setItem("dusk-discord-rpc-enabled", "false");
    setDiscordRpcEnabled(false);
    try {
      await api.discordRpcDisable();
      showToast("Dusk Rich Presence disabled.");
    } catch (error) {
      showToast(readableError(error), "error");
    }
  }

  const refreshCore = useCallback(
    async (initial = false) => {
      if (refreshCoreBusyRef.current) {
        refreshCorePendingRef.current = true;
        return;
      }

      refreshCoreBusyRef.current = true;
      if (initial) setLoading(true);

      try {
        do {
          refreshCorePendingRef.current = false;
          // A hung local database query should show a recoverable error
          // rather than leave the main Dusk content on a perpetual loader.
          const load = Promise.all([
            api.listGames(),
            api.getStats(),
            api.listAchievements(),
            api.listCollections(),
            api.collectionMemberships(),
            api.listProfiles(),
            api.getActiveProfile(),
          ]);
          let timeoutId: number | undefined;
          const deadline = new Promise<never>((_, reject) => {
            timeoutId = window.setTimeout(
              () => reject(new Error("Dusk's library did not respond. You can still use Settings or retry the scan.")),
              12_000,
            );
          });
          const values = await Promise.race([load, deadline]).finally(() => {
            if (timeoutId !== undefined) window.clearTimeout(timeoutId);
          });
          setGames(values[0]);
          setStats(values[1]);
          setAchievements(values[2]);
          setCollections(values[3]);
          setMemberships(values[4]);
          setProfiles(values[5]);
          setActiveProfile(values[6]);
          void syncAccountState().catch(() => undefined);
        } while (refreshCorePendingRef.current);
      } catch (error) {
        showToast(readableError(error), "error");
      } finally {
        refreshCoreBusyRef.current = false;
        if (initial) setLoading(false);
      }
    },
    [showToast],
  );

  const refreshScreenshots = useCallback(async () => {
    if (refreshScreenshotsBusyRef.current) {
      refreshScreenshotsPendingRef.current = true;
      return;
    }

    refreshScreenshotsBusyRef.current = true;
    try {
      do {
        refreshScreenshotsPendingRef.current = false;
        setScreenshots(await api.listScreenshots());
        screenshotsLoadedRef.current = true;
      } while (refreshScreenshotsPendingRef.current);
    } catch (error) {
      showToast(readableError(error), "error");
    } finally {
      refreshScreenshotsBusyRef.current = false;
    }
  }, [showToast]);

  useEffect(() => {
    void refreshCore(true)
      .then(() => api.refreshMissingCovers())
      .then((result) => {
        if (result.updated > 0) {
          return refreshCore(false);
        }
        return undefined;
      })
      .catch(() => undefined);
    void api.dataDirectory().then(setDataDirectory).catch(() => undefined);
    void cloudSaveStatus().then(setCloudStatus);
    const syncTimer = window.setInterval(() => {
      void syncAccountState().catch(() => undefined);
    }, 30000);
    return () => window.clearInterval(syncTimer);
  }, [refreshCore]);

  useEffect(() => {
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void checkForDuskUpdate()
        .then((result) => {
          if (!cancelled && result.available && result.version) {
            setAvailableUpdate(result.version);
            setAvailableUpdateNotes(result.body || null);
            showToast("Dusk " + result.version + " is available. See Settings → Updates for the changelog.");
          }
        })
        .catch(() => undefined);
    }, 2500);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, []);

  useEffect(() => {
    if (localStorage.getItem("dusk-account-mode") === "guest") {
      setAccountIdentity(null);
      return;
    }

    void currentDuskAccount()
      .then(setAccountIdentity)
      .catch(() => setAccountIdentity(null));

    const onAvatarChanged = (event: Event) => {
      const detail = (event as CustomEvent<DuskAccount>).detail;
      if (detail) setAccountIdentity(detail);
    };
    window.addEventListener("dusk-account-avatar-changed", onAvatarChanged);
    return () => window.removeEventListener("dusk-account-avatar-changed", onAvatarChanged);
  }, []);

  useEffect(() => {
    void getCurrentWindow()
      .isMaximized()
      .then(setWindowMaximized)
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!autoScanEnabled) return;

    const run = () => void autoScanGames(false);
    const startup = window.setTimeout(run, 1200);
    autoScanTimerRef.current = window.setInterval(run, 10 * 60 * 1000);

    const onVisibility = () => {
      if (document.visibilityState !== "visible") return;
      const last = Number(localStorage.getItem("dusk-last-auto-scan") || "0");
      if (Date.now() - last >= 10 * 60 * 1000) {
        void autoScanGames(false);
      }
    };

    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.clearTimeout(startup);
      if (autoScanTimerRef.current !== null) {
        window.clearInterval(autoScanTimerRef.current);
        autoScanTimerRef.current = null;
      }
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [autoScanEnabled]);

  useEffect(() => {
    if (view === "screenshots" && !screenshotsLoadedRef.current) {
      void refreshScreenshots();
    }
  }, [view, refreshScreenshots]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      void refreshCore(false);
    }, 30000);
    return () => window.clearInterval(timer);
  }, [refreshCore]);

  useEffect(() => {
    if (!discordRpcEnabled) return;
    const clientId = localStorage.getItem("dusk-discord-client-id") || discordClientId.trim();
    if (!/^\d{15,24}$/.test(clientId)) return;

    void api.discordRpcEnable(clientId, discordPresenceState()).catch(() => {
      localStorage.setItem("dusk-discord-rpc-enabled", "false");
      setDiscordRpcEnabled(false);
    });
  }, []);

  useEffect(() => {
    if (!discordRpcEnabled) return;
    void api.discordRpcUpdate(discordPresenceState()).catch(() => undefined);
  }, [discordRpcEnabled, view, playingId]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.accent = accent;
    localStorage.setItem("dusk-theme", theme);
    localStorage.setItem("dusk-accent", accent);
  }, [theme, accent]);

  async function minimizeWindow() {
    if (windowActionLockRef.current) return;
    windowActionLockRef.current = true;
    try {
      await getCurrentWindow().minimize();
    } catch (error) {
      showToast("Could not minimize Dusk: " + readableError(error), "error");
    } finally {
      windowActionLockRef.current = false;
    }
  }

  async function toggleWindowMaximize() {
    if (windowActionLockRef.current) return;
    windowActionLockRef.current = true;
    try {
      const appWindow = getCurrentWindow();
      await appWindow.toggleMaximize();
      setWindowMaximized(await appWindow.isMaximized());
    } catch (error) {
      showToast("Could not resize Dusk: " + readableError(error), "error");
    } finally {
      windowActionLockRef.current = false;
    }
  }

  async function closeWindow() {
    if (windowActionLockRef.current) return;
    windowActionLockRef.current = true;
    try {
      await getCurrentWindow().close();
    } catch (error) {
      windowActionLockRef.current = false;
      showToast("Could not close Dusk: " + readableError(error), "error");
    }
  }

  async function handleTitlebarMouseDown(event: MouseEvent<HTMLElement>) {
    if (event.button !== 0) return;
    const target = event.target;
    if (target instanceof HTMLElement && target.closest("button, input, select, a")) return;

    if (event.detail === 2) {
      await toggleWindowMaximize();
      return;
    }

    try {
      await getCurrentWindow().startDragging();
    } catch {
      // Dragging is best-effort; window buttons remain available.
    }
  }

  async function setConsoleModeEnabled(enabled: boolean) {
    if (consoleModeLockRef.current) return;
    consoleModeLockRef.current = true;
    try {
      await getCurrentWindow().setFullscreen(enabled);
      setConsoleMode(enabled);
      localStorage.setItem("dusk-console-mode", String(enabled));

      if (enabled) {
        window.setTimeout(() => {
          const first = document.querySelector<HTMLElement>(
            '.game-card, .nav-item, button:not(:disabled)',
          );
          first?.focus();
        }, 180);
      }
    } catch (error) {
      showToast("Could not change fullscreen mode: " + readableError(error), "error");
    } finally {
      consoleModeLockRef.current = false;
    }
  }

  function controllerBack() {
    if (selectedGameId) {
      setSelectedGameId(null);
      return;
    }
    if (addOpen) {
      setAddOpen(false);
      return;
    }
    if (view !== "home") {
      setView("home");
      setCollectionFilter("all");
    }
  }

  function controllerPlay(gameId: string) {
    const game = games.find((item) => item.id === gameId);
    if (game) void play(game);
  }

  const { connected: controllerConnected } = useControllerNavigation({
    onBack: controllerBack,
    onToggleConsole: () => void setConsoleModeEnabled(!consoleMode),
    onPlayGame: controllerPlay,
  });

  useEffect(() => {
    if (!consoleMode) return;
    void getCurrentWindow().setFullscreen(true).catch(() => undefined);
  }, []);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "F11") {
        event.preventDefault();
        void setConsoleModeEnabled(!consoleMode);
      } else if (event.key === "Escape" && consoleMode) {
        event.preventDefault();
        controllerBack();
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [consoleMode, selectedGameId, addOpen, view, games]);


  const selectedGame = games.find((game) => game.id === selectedGameId) || null;

  const membershipLookup = useMemo(() => {
    const map = new Map<string, Set<string>>();
    for (const membership of memberships) {
      const set = map.get(membership.collectionId) || new Set<string>();
      set.add(membership.gameId);
      map.set(membership.collectionId, set);
    }
    return map;
  }, [memberships]);

  const visibleGames = useMemo(() => {
    let next = [...games];

    if (view === "favorites") next = next.filter((game) => game.favorite);

    if (collectionFilter !== "all") {
      const allowed = membershipLookup.get(collectionFilter) || new Set<string>();
      next = next.filter((game) => allowed.has(game.id));
    }

    if (sourceFilter !== "all") {
      next = next.filter((game) => game.source === sourceFilter);
    }

    const needle = webSearchEnabled ? "" : query.trim().toLocaleLowerCase();
    if (needle) {
      next = next.filter((game) =>
        (game.title + " " + game.source).toLocaleLowerCase().includes(needle),
      );
    }

    next.sort((a, b) => {
      if (sortMode === "playtime") return b.totalSeconds - a.totalSeconds;
      if (sortMode === "recent") {
        return new Date(b.lastPlayed || 0).getTime() - new Date(a.lastPlayed || 0).getTime();
      }
      return a.title.localeCompare(b.title);
    });

    return next;
  }, [games, view, collectionFilter, membershipLookup, sourceFilter, query, sortMode, webSearchEnabled]);

  const recentGames = useMemo(() => {
    return [...games]
      .filter((game) => Boolean(game.lastPlayed))
      .sort(
        (a, b) =>
          new Date(b.lastPlayed || 0).getTime() - new Date(a.lastPlayed || 0).getTime(),
      )
      .slice(0, 6);
  }, [games]);

  const favoriteGames = useMemo(() => games.filter((game) => game.favorite).slice(0, 6), [games]);

  async function autoScanGames(force: boolean) {
    if ((!autoScanEnabled && !force) || scanLockRef.current) return;

    const last = Number(localStorage.getItem("dusk-last-auto-scan") || "0");
    if (!force && last > 0 && Date.now() - last < 10 * 60 * 1000) return;

    scanLockRef.current = true;
    setScanning(true);

    try {
      const result = await api.scanGames();
      const timestamp = Date.now();
      localStorage.setItem("dusk-last-auto-scan", String(timestamp));
      setLastAutoScanAt(timestamp);
      await refreshCore(false);

      if (result.added > 0) {
        showToast(
          "Automatic scan found " +
            String(result.added) +
            " new game" +
            (result.added === 1 ? "." : "s."),
        );
      }
    } catch (error) {
      // Automatic discovery should never interrupt normal use.
      console.warn("Automatic game scan failed:", error);
    } finally {
      setScanning(false);
      scanLockRef.current = false;
    }
  }

  async function scan() {
    if (scanLockRef.current) return;
    scanLockRef.current = true;
    setScanning(true);

    try {
      const result = await api.scanGames();

      let screenshotResult = null;
      if (!screenshotScanLockRef.current) {
        screenshotScanLockRef.current = true;
        try {
          screenshotResult = await api.scanScreenshots();
        } finally {
          screenshotScanLockRef.current = false;
        }
      }

      await Promise.all([refreshCore(false), refreshScreenshots()]);
      const detail =
        String(result.added) +
        " new · " +
        String(result.updated) +
        " refreshed · " +
        String(result.steamFound) +
        " Steam · " +
        String(result.epicFound) +
        " Epic · " +
        String(result.gogFound) +
        " GOG · " +
        String(result.emulatorFound) +
        " emulators · " +
        String(result.deviceFound) +
        " device folders" +
        (screenshotResult && screenshotResult.imported > 0
          ? " · " + String(screenshotResult.imported) + " screenshots"
          : "");
      const timestamp = Date.now();
      localStorage.setItem("dusk-last-auto-scan", String(timestamp));
      setLastAutoScanAt(timestamp);
      showToast("Scan complete: " + String(result.found) + " found (" + detail + ").");
      if (result.warnings.length > 0 && result.found === 0) {
        window.setTimeout(() => showToast(result.warnings[0], "error"), 500);
      }
    } catch (error) {
      showToast(readableError(error), "error");
    } finally {
      setScanning(false);
      scanLockRef.current = false;
    }
  }

  async function scanScreenshotsNow() {
    if (screenshotScanLockRef.current) return;
    screenshotScanLockRef.current = true;
    setScreenshotScanning(true);

    try {
      const result = await api.scanScreenshots();
      await Promise.all([refreshScreenshots(), refreshCore(false)]);
      if (result.imported > 0) {
        showToast(
          "Found " +
            String(result.imported) +
            " new screenshot" +
            (result.imported === 1 ? "." : "s.") +
            " Steam: " +
            String(result.steamImported) +
            " · matched folders: " +
            String(result.matchedImported) +
            ".",
        );
      } else if (result.found > 0) {
        showToast("Screenshot scan complete. No new screenshots; existing files were already known.");
      } else {
        showToast("Screenshot scan complete. No matching screenshots were found.");
      }
    } catch (error) {
      showToast(readableError(error), "error");
    } finally {
      setScreenshotScanning(false);
      screenshotScanLockRef.current = false;
    }
  }

  async function play(game: GameRecord) {
    if (playingIdsRef.current.has(game.id)) return;
    playingIdsRef.current.add(game.id);
    setPlayingId(game.id);

    try {
      const result = await api.launchGame(game.id);
      showToast(result.message, result.started ? "ok" : "error");
    } catch (error) {
      showToast(readableError(error), "error");
    } finally {
      playingIdsRef.current.delete(game.id);
      setPlayingId((current) => (current === game.id ? null : current));
    }
  }

  async function toggleFavorite(game: GameRecord) {
    if (favoriteLocksRef.current.has(game.id)) return;
    favoriteLocksRef.current.add(game.id);

    const nextFavorite = !game.favorite;
    setGames((current) =>
      current.map((item) =>
        item.id === game.id ? { ...item, favorite: nextFavorite } : item,
      ),
    );
    setStats((current) => ({
      ...current,
      favoriteCount: Math.max(0, current.favoriteCount + (nextFavorite ? 1 : -1)),
    }));

    try {
      await api.setFavorite(game.id, nextFavorite);
    } catch (error) {
      setGames((current) =>
        current.map((item) =>
          item.id === game.id ? { ...item, favorite: game.favorite } : item,
        ),
      );
      void refreshCore(false);
      showToast(readableError(error), "error");
    } finally {
      favoriteLocksRef.current.delete(game.id);
    }
  }

  async function syncEveryOwnerToCloud() {
    if (cloudSyncBusy) return;
    setCloudSyncBusy(true);
    try {
      const status = await cloudSaveStatus();
      setCloudStatus(status);
      if (!status.configured || !status.authenticated) {
        showToast(status.message, "error");
        return;
      }

      const result = await syncAllBackupsToSupabase();
      await syncCloudManifest();
      showToast(
        "Supabase sync complete: " +
          String(result.backups) +
          " backups · " +
          String(result.uploadedFiles) +
          " files · " +
          formatBytes(result.uploadedBytes) +
          ".",
      );
      setCloudStatus(await cloudSaveStatus());
    } catch (error) {
      showToast(readableError(error), "error");
    } finally {
      setCloudSyncBusy(false);
    }
  }

  async function switchOwnerProfile(profileId: string) {
    if (profileId === activeProfile?.id) return;
    try {
      const profile = await api.setActiveProfile(profileId);
      setActiveProfile(profile);
      setSelectedGameId(null);
      setCollectionFilter("all");
      await refreshCore(false);
      try {
        await syncCloudManifest();
      } catch {
        // Cloud sync is optional; profile switching remains local-first.
      }
      showToast("Switched to " + profile.name + ".");
    } catch (error) {
      showToast(readableError(error), "error");
    }
  }

  async function createOwnerProfile() {
    const name = window.prompt("Profile / owner name");
    if (!name || !name.trim()) return;
    try {
      const profile = await api.createProfile(name.trim());
      setActiveProfile(profile);
      setSelectedGameId(null);
      setCollectionFilter("all");
      await refreshCore(false);
      try {
        await syncCloudManifest();
      } catch {
        // Profile is still saved locally if cloud is unavailable.
      }
      showToast("Created owner profile " + profile.name + ".");
    } catch (error) {
      showToast(readableError(error), "error");
    }
  }

  async function renameOwnerProfile(profile: ProfileRecord) {
    const name = window.prompt("Rename owner profile", profile.name);
    if (!name || !name.trim() || name.trim() === profile.name) return;

    try {
      const renamed = await api.renameProfile(profile.id, name.trim());
      if (activeProfile?.id === renamed.id) setActiveProfile(renamed);
      await refreshCore(false);
      try {
        await syncAccountState();
        await syncCloudManifest();
      } catch {
        // Renaming is local-first and will sync the next time cloud sync succeeds.
      }
      showToast("Renamed profile to " + renamed.name + ".");
    } catch (error) {
      showToast(readableError(error), "error");
    }
  }

  async function removeOwnerProfile(profile: ProfileRecord) {
    if (profiles.length <= 1) {
      showToast("Dusk must keep at least one owner profile.", "error");
      return;
    }
    const okay = window.confirm(
      "Delete profile " +
        profile.name +
        "? Its local Dusk backup copies will be removed, but live game save folders are not deleted.",
    );
    if (!okay) return;
    try {
      const fallback = await api.deleteProfile(profile.id);
      setActiveProfile(fallback);
      setSelectedGameId(null);
      await refreshCore(false);
      try {
        await syncCloudManifest();
      } catch {
        // Local deletion remains valid if cloud is unavailable.
      }
      showToast("Profile deleted.");
    } catch (error) {
      showToast(readableError(error), "error");
    }
  }

  async function createCollection() {
    if (collectionMutationRef.current) return;
    const name = window.prompt("Collection name");
    if (!name || !name.trim()) return;

    collectionMutationRef.current = true;
    try {
      const collection = await api.createCollection(name.trim());
      await refreshCore(false);
      setCollectionFilter(collection.id);
      setView("library");
      showToast("Created " + collection.name + ".");
    } catch (error) {
      showToast(readableError(error), "error");
    } finally {
      collectionMutationRef.current = false;
    }
  }

  async function deleteCollection(collection: CollectionRecord) {
    if (collectionMutationRef.current) return;
    const okay = window.confirm(
      'Delete the "' + collection.name + '" collection? Games stay in your library.',
    );
    if (!okay) return;

    collectionMutationRef.current = true;
    try {
      await api.deleteCollection(collection.id);
      if (collectionFilter === collection.id) setCollectionFilter("all");
      await refreshCore(false);
      showToast("Collection deleted.");
    } catch (error) {
      showToast(readableError(error), "error");
    } finally {
      collectionMutationRef.current = false;
    }
  }

  async function checkUpdates() {
    if (updateLockRef.current) return;
    updateLockRef.current = true;
    setUpdateBusy(true);

    try {
      const result = await checkForDuskUpdate();
      if (result.available && result.version) {
        setAvailableUpdate(result.version);
        setAvailableUpdateNotes(result.body || null);
        showToast("Dusk " + result.version + " is available. Changelog: Settings → Updates.");
      } else {
        setAvailableUpdate(null);
        setAvailableUpdateNotes(null);
        showToast("Dusk is up to date.");
      }
    } catch (error) {
      showToast(
        "Could not check for Dusk updates: " + readableError(error),
        "error",
      );
    } finally {
      updateLockRef.current = false;
      setUpdateBusy(false);
    }
  }

  async function installUpdate() {
    if (updateLockRef.current) return;
    updateLockRef.current = true;
    setUpdateBusy(true);

    try {
      await installDuskUpdate();
    } catch (error) {
      showToast(readableError(error), "error");
    } finally {
      updateLockRef.current = false;
      setUpdateBusy(false);
    }
  }

  async function finishGameImport(result: { directory: string; game: GameRecord | null; installers: string[] }, background = false) {
    if (result.game) {
      await refreshCore(false);
      showToast("Extracted and added " + result.game.title + " to your Dusk library.");
      return;
    }
    if (result.installers.length === 1) {
      showToast("Files extracted to " + result.directory + ". Installer awaiting confirmation.");
      if (!background && window.confirm("Run the extracted installer? Only proceed if you trust the downloaded files. Dusk will not bypass Windows security warnings.")) {
        await api.runGameInstaller(result.installers[0]);
        showToast("Installer launched. Dusk will detect the game on the next library scan after setup finishes.");
      }
    } else if (result.installers.length > 1) {
      showToast("Multiple installers found in " + result.directory + ". Choose the correct installer manually.", "error");
    } else {
      showToast("Files extracted to " + result.directory + ". Choose the game's executable with Add game.");
    }
  }

  // Native transfers are polled while Dusk runs; the visible game pages do not
  // need to stay open. Completed single-volume archives import automatically.
  useEffect(() => {
    let mounted = true;
    const refresh = async () => {
      try {
        const items = await api.listManagedDownloads();
        if (mounted) {
          managedDownloadPathsRef.current = new Set(items.map((item) => item.filePath.toLowerCase()));
          setManagedDownloads(items);
        }
      } catch { /* Downloads must not prevent the game library from loading. */ }
    };
    void refresh();
    const poll = window.setInterval(() => void refresh(), 2000);
    return () => { mounted = false; window.clearInterval(poll); };
  }, []);

  useEffect(() => {
    for (const item of managedDownloads) {
      if (item.status === "failed" && !failedDownloadIdsRef.current.has(item.id)) {
        failedDownloadIdsRef.current.add(item.id);
        const detail = item.error || "File host refused the transfer.";
        const hostBlocked = /404|403|file host|web page|access denied|download mirror|archive/i.test(detail);
        if (item.listingUrl && hostBlocked) {
          showToast("No usable download mirror for " + item.title + ". Opening the original listing in Dusk. " + detail, "error");
          void api.openGameSourceListing(item.listingUrl!)
            .catch(error => showToast("Could not open listing: " + readableError(error), "error"));
        } else {
          showToast("Download failed for " + item.title + ": " + detail, "error");
        }
      }
    }
    if (importingInBackgroundRef.current) return;

    const ready = managedDownloads.find((item) =>
      item.status === "completed" && !handledDownloadIdsRef.current.has(item.id));
    if (!ready) return;

    handledDownloadIdsRef.current.add(ready.id);
    // A first multipart volume is not a complete game. Never try to unpack
    // an incomplete set of volumes or execute files from an archive.
    if (/\.7z\.\d{3}$|\.part\d+\.rar$/i.test(ready.filename) && !ready.bundleReady) {
      showToast("Downloaded " + ready.filename + ". Multipart archives need all volumes before import.");
      return;
    }

    importingInBackgroundRef.current = true;
    void (async () => {
      const imported = await api.importDownloadedGameArchive(
        ready.filePath, ready.title, ready.source === "online-fix" ? "online-fix.me" : undefined
      );
      if (imported.game) {
        await refreshCore(false);
        showToast(ready.title + " was downloaded and added to your library.");
      } else if (imported.installers.length === 1) {
        // Background completion must never open a blocking native confirm().
        // A hidden WebView2 dialog can make the entire main Dusk UI appear
        // unclickable while the separate browser popups keep working.
        await finishGameImport(imported, true);
      } else if (imported.installers.length > 1) {
        showToast(ready.title + " extracted. Multiple installers found; choose the correct one with Install game.");
      } else {
        showToast(ready.title + " extracted. No executable was detected; use Add game to select one.");
      }
    })().catch((error: unknown) => {
      showToast("Could not import " + ready.title + ": " + readableError(error), "error");
    }).finally(() => { importingInBackgroundRef.current = false; });
  }, [managedDownloads]);

  async function importArchive() {
    if (archiveBusy) return;
    setArchiveBusy(true);
    try {
      const result = await api.importGameArchive();
      if (result) await finishGameImport(result);
    } catch (error) {
      showToast(readableError(error), "error");
    } finally {
      setArchiveBusy(false);
    }
  }

  async function trackOnlineFixDownload(result: WebGameResult, externalBrowser = false, destination = result.url, autoSelect = false) {
    downloadStabilityRef.current.clear();
    watchAttemptedRef.current.clear();
    watchVerificationCheckedRef.current.clear();
    watchImportRetryAtRef.current.clear();
    setDownloadWatchMessage("Waiting for a completed archive matching " + result.title + " in Downloads…");
    // Start monitoring before opening the selected verified download page.
    setActiveDownloadWatch({ sinceMs: Date.now() - 2000, title: result.title, url: destination, source: "online-fix" });
    try {
      if (externalBrowser) await api.openOnlineFixBrowser(destination);
      else await api.openOnlineFixResult(destination, result.title, autoSelect);
    } catch (error) {
      setActiveDownloadWatch(null);
      setDownloadWatchMessage("");
      showToast(readableError(error), "error");
    }
  }

  async function openOtherGameSource(result: WebGameResult, external = false) {
    // Browsing still happens on the real host. A completed, title-matching
    // archive is picked up and imported without a manual URL-paste form.
    downloadStabilityRef.current.clear();
    watchAttemptedRef.current.clear();
    watchVerificationCheckedRef.current.clear();
    watchImportRetryAtRef.current.clear();
    setDownloadWatchMessage("Watching for a download of " + result.title + " in the background…");
    setActiveDownloadWatch({
      sinceMs: Date.now() - 2000,
      title: result.title,
      url: result.url,
      source: webSource,
    });
    try {
      if (external) await api.openGameSourceBrowser(result.url);
      else await api.openGameSourceListing(result.url);
    } catch (error) {
      setActiveDownloadWatch(null);
      setDownloadWatchMessage("");
      showToast(readableError(error), "error");
    }
  }

  async function startHosterFileDownload(result: WebGameResult, file: OnlineFixHosterFile) {
    if (file.isFix) {
      showToast("This is a fix/update archive, not the full game.", "error");
      return;
    }
    if (file.requiresCaution && !window.confirm("This provider is marked as potentially risky by the site. Continue to the host?")) return;
    if (!file.directArchive) {
      await trackOnlineFixDownload(result, true, file.url);
      return;
    }
    try {
      const created = await api.startManagedDownload(file.url, file.filename, result.title, "online-fix");
      setManagedDownloads((current) => [created, ...current.filter((entry) => entry.id !== created.id)]);
      showToast("Downloading " + file.filename + " from " + file.provider + " in the background.");
    } catch (error) {
      showToast("Download could not start: " + readableError(error), "error");
    }
  }

  function findHosterMultipartBundle(files: OnlineFixHosterFile[]) {
    const groups = new Map<string, Map<number, { filename: string; mirrors: string[] }>>();
    for (const file of files) {
      if (!file.directArchive || file.isFix || file.requiresCaution) continue;
      const rar = file.filename.match(/^(.*)\.part0*(\d+)\.rar$/i);
      const seven = file.filename.match(/^(.*\.7z)\.(\d{3})$/i);
      const match = rar || seven;
      if (!match) continue;
      const volume = Number(match[2]);
      if (!Number.isInteger(volume) || volume < 1 || volume > 120) continue;
      const groupId = (match[1] + (rar ? ".rar" : "")).toLocaleLowerCase();
      if (!groups.has(groupId)) groups.set(groupId, new Map());
      const group = groups.get(groupId)!;
      if (!group.has(volume)) group.set(volume, { filename: file.filename, mirrors: [] });
      const entry = group.get(volume)!;
      if (!entry.mirrors.includes(file.url) && entry.mirrors.length < 8) entry.mirrors.push(file.url);
    }
    const complete = Array.from(groups.values())
      .map(group => Array.from(group.entries()).sort(([a], [b]) => a - b))
      .filter(entries => entries.length >= 2 && entries.every(([number], index) => number === index + 1))
      .sort((a, b) => b.length - a.length);
    return complete.length ? complete[0].map(([, part]) => part) : null;
  }

  async function selectGameDownload(result: WebGameResult) {
    if (downloadSourcesBusy) return;
    setDownloadSourcesBusy(result.url);
    try {
      const links = await api.getOnlineFixDownloadLinks(result.url);
      setDownloadSources((current) => ({ ...current, [result.url]: links }));
      // Hosters exposes the actual files, not just a folder page. In contrast,
      // Drive can require authorization, so do not silently open a 401 page.
      const hosters = links.find((link) => link.url.startsWith("https://hosters.online-fix.me"));
      if (hosters) {
        try {
          const files = await api.getOnlineFixHosterFiles(hosters.url);
          setHosterFiles((current) => ({ ...current, [result.url]: files }));
          const multipart = findHosterMultipartBundle(files);
          if (multipart) {
            const created = await api.startArchiveBundle(result.title, multipart, undefined, "online-fix");
            setManagedDownloads(current => [created, ...current.filter(item => item.id !== created.id)]);
            showToast("Downloading " + multipart.length + " archive volumes with automatic mirror fallback.");
            return;
          }
          const primary = files.find(file => file.directArchive && !file.isFix && !file.requiresCaution &&
            !/\.7z\.\d{3}$|\.part\d+\.rar$/i.test(file.filename));
          if (primary) {
            await startHosterFileDownload(result, primary);
            return;
          }
          showToast("Choose a game archive from the Hosters files below.");
          return;
        } catch (error) {
          showToast("Native file discovery failed: " + readableError(error) + ". Opening Hosters directly.");
          await trackOnlineFixDownload(result, false, hosters.url, false);
          return;
        }
      }
      const drive = links.find((link) => link.url.startsWith("https://drive.online-fix.me"));
      if (drive) {
        showToast("Hosters was not listed. Opening Drive; account authorization may be required.");
        await trackOnlineFixDownload(result, false, drive.url, false);
        return;
      }
      showToast("No recognized download sources found. Opening the game listing.");
      await trackOnlineFixDownload(result, false, result.url, false);
    } catch (error) {
      showToast("Could not load game files: " + readableError(error), "error");
    } finally {
      setDownloadSourcesBusy(null);
    }
  }

  // One click finds direct archive files on the selected listing. Multiple
  // URLs for the same volume are mirrors; numbered volumes are all required.
  async function oneClickGameSourceDownload(result: WebGameResult) {
    if (downloadSourcesBusy || webSource === "online-fix") return;
    const source = webSource;
    setDownloadSourcesBusy(result.url);
    try {
      const bundles = await api.discoverGameSourceArchives(source, result.url);
      if (!bundles.length) {
        showToast("No working direct archive mirror was verified (the host may return 404, a web page, or require extra steps). Opening the listing in Dusk.");
        await openOtherGameSource(result);
        return;
      }
      // Prefer the most complete contiguous volume set. Alternatives/mirrors
      // are never mistaken for extra volumes of the same archive.
      const selected = bundles[0];
      const created = await api.startArchiveBundle(result.title, selected.parts, result.url);
      setManagedDownloads(current => [created, ...current.filter(item => item.id !== created.id)]);
      showToast("Downloading " + selected.parts.length + " archive volume(s) with automatic mirror fallback.");
    } catch (error) {
      showToast("Direct download could not be verified: " + readableError(error) + ". Opening the listing in Dusk.", "error");
      await openOtherGameSource(result);
    } finally {
      setDownloadSourcesBusy(null);
    }
  }

  async function runLocalInstaller() {
    if (installerLockRef.current) return;

    installerLockRef.current = true;
    setInstallerBusy(true);
    try {
      const installerPath = await api.chooseGameInstaller();
      if (!installerPath) return;

      await api.runGameInstaller(installerPath);
      showToast("Installer launched. Dusk will discover the game on the next automatic scan.");
    } catch (error) {
      showToast(readableError(error), "error");
    } finally {
      installerLockRef.current = false;
      setInstallerBusy(false);
    }
  }

  async function searchWeb() {
    const term = query.trim();
    if (!term || webLoading) return;
    setWebLoading(true);
    setWebSearched(true);
    setWebError("");
    setWebResults([]);
    try {
      const results = webSource === "online-fix"
        ? await api.searchOnlineFixGames(term)
        : await api.searchGameSource(webSource, term);
      setWebResults(results);
    } catch (error) {
      setWebError(readableError(error));
    } finally {
      setWebLoading(false);
    }
  }

  async function openExternal(target: "creator" | "steam" | "epic" | "gog" | "itch") {
    try {
      await api.openExternalTarget(target);
    } catch (error) {
      showToast(readableError(error), "error");
    }
  }

  async function deleteScreenshot(screenshot: ScreenshotRecord) {
    if (screenshotDeleteLocksRef.current.has(screenshot.id)) return;
    if (!window.confirm("Delete this imported screenshot from Dusk?")) return;

    screenshotDeleteLocksRef.current.add(screenshot.id);
    try {
      await api.deleteScreenshot(screenshot.id);
      setScreenshots((current) => current.filter((item) => item.id !== screenshot.id));
      setStats((current) => ({
        ...current,
        screenshotCount: Math.max(0, current.screenshotCount - 1),
      }));
      showToast("Screenshot deleted.");
    } catch (error) {
      void refreshScreenshots();
      showToast(readableError(error), "error");
    } finally {
      screenshotDeleteLocksRef.current.delete(screenshot.id);
    }
  }

  const activeCollection = collections.find((item) => item.id === collectionFilter);
  const libraryTitle =
    view === "favorites"
      ? "Favorites"
      : collectionFilter !== "all"
        ? activeCollection?.name || "Collection"
        : "Library";

  // One tracked download at a time; this never imports files that predate opening a listing.
  useEffect(() => {
    if (!activeDownloadWatch) return;
    let cancelled = false;
    const watch = activeDownloadWatch;
    const poll = async () => {
      if (cancelled || watchBusyRef.current) return;
      watchBusyRef.current = true;
      try {
        if (Date.now() - watch.sinceMs > 2 * 60 * 60 * 1000) {
          setActiveDownloadWatch(null);
          setDownloadWatchMessage("Download monitoring stopped after two hours.");
          return;
        }
        const archives = await api.listRecentGameArchives(watch.sinceMs);
        if (cancelled) return;
        for (const candidate of archives) {
          if (managedDownloadPathsRef.current.has(candidate.path.toLowerCase())) continue;
          if (!matchesGameArchive(candidate.filename, watch.title)) continue;
          const prior = downloadStabilityRef.current.get(candidate.path);
          const stable = prior && prior.size === candidate.sizeBytes ? prior.stable + 1 : 0;
          downloadStabilityRef.current.set(candidate.path, { size: candidate.sizeBytes, stable });
          const key = candidate.path + ":" + candidate.sizeBytes;
          if (stable < 2 || watchAttemptedRef.current.has(key)) continue;
          if (Date.now() < (watchImportRetryAtRef.current.get(key) || 0)) continue;
          const isMultipart = /\.7z\.001$|\.part0*1\.rar$/i.test(candidate.filename);
          if (isMultipart) {
            const lastChecked = watchVerificationCheckedRef.current.get(key) || 0;
            if (Date.now() - lastChecked < 60_000) continue;
            watchVerificationCheckedRef.current.set(key, Date.now());
            setDownloadWatchMessage("Checking archive integrity before extraction: " + candidate.filename + "…");
            const verified = await api.verifyDownloadedMultipartArchive(
              candidate.path, watch.source === "online-fix" ? "online-fix.me" : undefined,
            );
            if (cancelled) return;
            if (!verified.ready) {
              setDownloadWatchMessage(verified.message + " Monitoring Downloads for more parts.");
              continue;
            }
          }
          watchAttemptedRef.current.add(key);
          setDownloadWatchMessage("Archive ready: " + candidate.filename + ". Extracting into Dusk…");
          try {
            const imported = await api.importDownloadedGameArchive(candidate.path, watch.title, watch.source === "online-fix" ? "online-fix.me" : undefined);
            if (cancelled) return;
            setActiveDownloadWatch(null);
            setDownloadWatchMessage("Imported " + candidate.filename + ".");
            // The watcher runs without a direct user click; never show a
            // modal confirmation here or block interaction with the main UI.
            await finishGameImport(imported, true);
          } catch (error) {
            // A transient extraction error (disk space, a changing volume,
            // locked file) should not permanently suppress retrying this file.
            // Back off so a bad archive does not trigger costly extraction on
            // every six-second watcher poll.
            watchAttemptedRef.current.delete(key);
            watchImportRetryAtRef.current.set(key, Date.now() + 60_000);
            if (!cancelled) setDownloadWatchMessage(
              "Import failed: " + readableError(error) + " — retrying after 60 seconds."
            );
          }
          break;
        }
      } catch (error) {
        if (!cancelled) setDownloadWatchMessage("Could not check Downloads: " + readableError(error));
      } finally {
        watchBusyRef.current = false;
      }
    };
    void poll();
    const timer = window.setInterval(() => void poll(), 6000);
    return () => { cancelled = true; window.clearInterval(timer); };
  }, [activeDownloadWatch]);

  const guestMode = localStorage.getItem("dusk-account-mode") === "guest";

  return (
    <div className={cx("window-frame", consoleMode && "console-active")}>
      <header
        className="window-titlebar"
        data-tauri-drag-region
        onMouseDown={(event) => void handleTitlebarMouseDown(event)}
      >
        <div className="window-titlebar-spacer" data-tauri-drag-region aria-hidden="true" />
        <div className="window-title" data-tauri-drag-region>
          <img className="window-title-logo" src={duskLogo} alt="" draggable={false} />
          <span>Dusk</span>
        </div>
        <div className="window-controls">
          <button
            className="window-control"
            aria-label="Minimize Dusk"
            title="Minimize"
            onClick={() => void minimizeWindow()}
          >
            <Minus size={15} />
          </button>
          <button
            className="window-control"
            aria-label={windowMaximized ? "Restore Dusk" : "Maximize Dusk"}
            title={windowMaximized ? "Restore" : "Maximize"}
            onClick={() => void toggleWindowMaximize()}
          >
            <Square size={12} />
          </button>
          <button
            className="window-control close"
            aria-label="Close Dusk"
            title="Close"
            onClick={() => void closeWindow()}
          >
            <X size={15} />
          </button>
        </div>
      </header>

      <div className={cx("app-shell", consoleMode && "console-mode", controllerConnected && "controller-connected")}>
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">
            <img className="brand-logo-image" src={duskLogo} alt="" draggable={false} />
          </div>
          <div>
            <strong>Dusk</strong>
            <span>Game launcher</span>
          </div>
        </div>

        <nav className="nav-list" aria-label="Main navigation">
          <span className="sidebar-nav-label">Start</span>
          <NavItem
            active={view === "home"}
            icon={<Home size={18} />}
            label="Home"
            onClick={() => {
              setView("home");
              setCollectionFilter("all");
            }}
          />
          <span className="sidebar-nav-label">Play</span>
          <NavItem
            active={view === "library" && collectionFilter === "all"}
            icon={<Library size={18} />}
            label="Library"
            count={stats.gameCount}
            onClick={() => {
              setView("library");
              setCollectionFilter("all");
            }}
          />
          <NavItem
            active={view === "favorites"}
            icon={<Heart size={18} />}
            label="Favorites"
            count={stats.favoriteCount}
            onClick={() => {
              setView("favorites");
              setCollectionFilter("all");
            }}
          />
          <NavItem
            active={view === "trainerCatalog"}
            icon={<Gamepad2 size={18} />}
            label="Dusk Catalog"
            onClick={() => {
              setView("trainerCatalog");
              setWebSearched(false);
              void loadTrainerCatalog().catch(() => {});
            }}
          />
          <span className="sidebar-nav-label">Activity</span>
          <NavItem
            active={view === "screenshots"}
            icon={<Images size={18} />}
            label="Screenshots"
            count={stats.screenshotCount}
            onClick={() => setView("screenshots")}
          />
          <NavItem
            active={view === "achievements"}
            icon={<Trophy size={18} />}
            label="Achievements"
            onClick={() => setView("achievements")}
          />
        </nav>

        <div className="sidebar-divider" />

        <div className="sidebar-section-head">
          <span>Collections</span>
          <button className="mini-icon" onClick={createCollection} title="New collection">
            <Plus size={15} />
          </button>
        </div>

        <div className="collection-nav">
          {collections.length === 0 ? (
            <p className="sidebar-empty">No collections yet.</p>
          ) : (
            collections.map((collection) => (
              <div className="collection-row" key={collection.id}>
                <button
                  className={cx(
                    "collection-nav-item",
                    view === "library" && collectionFilter === collection.id && "active",
                  )}
                  onClick={() => {
                    setCollectionFilter(collection.id);
                    setView("library");
                  }}
                >
                  <Layers3 size={15} />
                  <span>{collection.name}</span>
                  <small>{collection.gameCount}</small>
                </button>
                <button
                  className="collection-delete"
                  onClick={() => void deleteCollection(collection)}
                  title={"Delete " + collection.name}
                >
                  <X size={13} />
                </button>
              </div>
            ))
          )}
        </div>

        <div className="sidebar-bottom">
          {!guestMode && accountIdentity && (
            <button
              className="sidebar-account-chip"
              type="button"
              onClick={() => setView("settings")}
              title="Edit Dusk account profile"
            >
              <div className="sidebar-account-avatar">
                {accountIdentity.avatarUrl ? (
                  <img src={accountIdentity.avatarUrl} alt="" />
                ) : (
                  <span>{(accountIdentity.displayName || accountIdentity.username || "D").charAt(0).toUpperCase()}</span>
                )}
              </div>
              <div className="sidebar-account-copy">
                <strong>{accountIdentity.displayName || accountIdentity.username}</strong>
                <small>@{accountIdentity.username}</small>
              </div>
            </button>
          )}
          <button
            className="creator-pill"
            onClick={() => void openExternal("creator")}
            title="Open guns.lol/bxane"
          >
            <span>bxane</span>
            <small>guns.lol/bxane</small>
            <ExternalLink size={12} />
          </button>
          <NavItem
            active={view === "settings"}
            icon={<Settings size={18} />}
            label="Settings"
            onClick={() => setView("settings")}
          />
          <div className="local-chip">
            <HardDrive size={14} />
            <span>Local-first</span>
          </div>
        </div>
      </aside>

      <main className="main">
        <header className="topbar">
          <div className="search-controls">
            <div className="search-box">
              {webSearchEnabled ? <Globe2 size={17} /> : <Search size={17} />}
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && webSearchEnabled) {
                    event.preventDefault();
                    void searchWeb();
                  }
                }}
                onFocus={() => {
                  if (!webSearchEnabled && view === "home") setView("library");
                }}
                placeholder={webSearchEnabled ? `Search ${WEB_SOURCES[webSource].label} (Enter)` : "Search your games"}
                aria-label={webSearchEnabled ? "Web search query" : "Search your games"}
              />
              {query && (
                <button type="button" onClick={() => setQuery("")} aria-label="Clear search">
                  <X size={15} />
                </button>
              )}
              {webSearchEnabled && (
                <button type="button" onClick={() => void searchWeb()} disabled={!query.trim()} aria-label="Search online games" title={`Search ${WEB_SOURCES[webSource].label} game listings inside Dusk`}>
                  <Search size={16} />
                </button>
              )}
            </div>
            <label className="web-search-toggle" title="Search the internet instead of filtering your game library">
              <input
                type="checkbox"
                checked={webSearchEnabled}
                onChange={(event) => { setWebSearchEnabled(event.target.checked); setWebSearched(false); setWebResults([]); setWebError(""); }}
              />
              <span className="web-search-switch" aria-hidden="true" />
              <span>Web search</span>
            </label>
            {webSearchEnabled && (
              <label className="web-source-control">
                <span>Source</span>
                <select className="web-source-select" value={webSource} disabled={webLoading}
                  aria-label="Game listing source"
                  onChange={(event) => {
                    setWebSource(event.target.value as WebGameSource);
                    setWebSearched(false);
                    setWebResults([]);
                    setWebError("");
                    setDownloadSources({});
                    setHosterFiles({});
                  }}>
                  <option value="online-fix">Online-Fix · Online</option>
                  <option value="game3rb">Game3rb · Games</option>
                  <option value="fitgirl">FitGirl · Offline only</option>
                </select>
              </label>
            )}
            {webSearchEnabled && (
              <span className="web-adblock-indicator"
                title="Ad and pop-up filtering is automatically enabled in all Dusk game browser windows">
                <ShieldCheck size={14} /> Ad blocking on
              </span>
            )}
          </div>

          <div className="top-actions">
            <button className="button secondary" onClick={() => void importArchive()} disabled={archiveBusy} title="Extract ZIP, RAR, or 7z archives with Python fallback">
              {archiveBusy ? <RefreshCw className="spin" size={16} /> : <Archive size={16} />}
              {archiveBusy ? "Extracting…" : "Import archive"}
            </button>
            <button className="button secondary" onClick={() => setAddOpen(true)}>
              <Plus size={16} />
              Add game
            </button>
            <button className="button primary" onClick={() => void scan()} disabled={scanning}>
              {scanning ? <RefreshCw className="spin" size={16} /> : <ScanSearch size={16} />}
              {scanning ? "Scanning…" : "Scan PC"}
            </button>
          </div>
        </header>

        <div className="content">
          {loading ? (
            <div className="loading-screen">
              <div className="brand-mark">
                <img className="brand-logo-image" src={duskLogo} alt="" draggable={false} />
              </div>
              <RefreshCw className="spin" size={20} />
              <span>Loading your local library…</span>
            </div>
          ) : webSearchEnabled && webSearched && view !== "trainerCatalog" ? (
            <section className="web-results-page">
              <div className="web-results-heading">
                <div>
                  <span className="eyebrow">{WEB_SOURCES[webSource].description}</span>
                  <h1>{WEB_SOURCES[webSource].label} game search</h1>
                  <p>Results for “{query.trim()}” from {WEB_SOURCES[webSource].host}.</p>
                </div>
                {webLoading && <RefreshCw className="spin" size={19} />}
              </div>
              {activeDownloadWatch && (
                <div className="download-watch-panel" role="status">
                  <RefreshCw className="spin" size={16} />
                  <div>
                    <strong>Tracking: {activeDownloadWatch.title}</strong>
                    <p>{downloadWatchMessage}</p>
                  </div>
                  <button className="button ghost" onClick={() => { setActiveDownloadWatch(null); setDownloadWatchMessage(""); }}>Stop</button>
                </div>
              )}
              {!activeDownloadWatch && downloadWatchMessage && <p className="web-results-note" role="status">{downloadWatchMessage}</p>}
              {webError && (
                <div className="inline-error" role="alert">
                  <p>{webError}</p>
                  {webSource !== "online-fix" && (
                    <button className="button secondary" onClick={() => {
                      void api.openGameSourceSearch(webSource, query.trim())
                        .catch(error => showToast(readableError(error), "error"));
                    }} title="Open this search in Dusk's embedded browser with its ad blocker enabled">
                      <Globe2 size={15} /> Open search in Dusk browser
                    </button>
                  )}
                </div>
              )}
              {!webLoading && !webError && webResults.length === 0 && (
                <p className="web-results-empty">No matching listings found. Try the full game title or another keyword.</p>
              )}
              <div className="web-results-list">
                {webResults.map((result) => (
                  <article className="web-result-card" key={result.url}>
                    <div>
                      <h3>{result.title}</h3>
                      {result.description && <p>{result.description}</p>}
                      <span>{WEB_SOURCES[webSource].host}{webSource === "fitgirl" ? " · Offline" : ""}</span>
                    </div>
                    {webSource === "online-fix" ? (
                    <div className="web-download-action-group">
                      <div className="web-result-actions">
                        <button className="button primary" disabled={downloadSourcesBusy !== null} onClick={() => void selectGameDownload(result)} title="Find the actual game archive on Hosters and start downloading in Dusk">
                          {downloadSourcesBusy === result.url ? <RefreshCw className="spin" size={15} /> : <Archive size={15} />}
                          {downloadSourcesBusy === result.url ? "Finding game file…" :
                            managedDownloads.some((download) => download.title === result.title && download.status === "downloading")
                              ? "Downloading in background" : "Get game"}
                        </button>
                        <button className="button secondary" onClick={() => void trackOnlineFixDownload(result)} title="Open the original game listing inside Dusk">
                          <Globe2 size={15} /> Listing
                        </button>
                        <button className="button ghost" onClick={() => void trackOnlineFixDownload(result, true)} title="Open the original listing in your regular browser">
                          <ExternalLink size={15} /> Browser
                        </button>
                      </div>
                      {hosterFiles[result.url] && (
                        <div className="web-hoster-files" aria-label="Actual game archives">
                          <strong>Game files on Hosters</strong>
                          {hosterFiles[result.url].filter((file) => !file.isFix).map((file) => (
                            <div className="web-hoster-file" key={file.url}>
                              <div className="web-hoster-file-info">
                                <span>{file.filename}</span>
                                <small>{file.provider}{file.requiresCaution ? " · Caution" : ""}</small>
                              </div>
                              <button className="button secondary"
                                onClick={() => void startHosterFileDownload(result, file)}
                                title={file.directArchive ? "Download this archive using Dusk's native download manager" : "This provider requires opening the file-host page"}>
                                {file.directArchive ? "Download in Dusk" : "Open file host"}
                              </button>
                              <button className="button ghost" onClick={() => void trackOnlineFixDownload(result, true, file.url)}
                                title="Open this precise archive's provider link in your regular browser">Browser</button>
                            </div>
                          ))}
                          {!hosterFiles[result.url].some((file) => !file.isFix) && (
                            <p>No complete game archives found; only fix/update files are listed.</p>
                          )}
                        </div>
                      )}
                      {downloadSources[result.url] && (
                        <div className="web-download-sources">
                          {downloadSources[result.url].filter((link) => link.kind === "game" || link.kind === "mirror").map((link) => (
                            <div className="web-download-source-group" key={link.url}>
                              <button className="web-download-source"
                                onClick={() => void trackOnlineFixDownload(result, false, link.url, true)}
                                title="Download via Dusk; files may come from a separate HTTPS CDN">
                                <Archive size={12} /> {link.label}
                              </button>
                              <button className="web-download-source"
                                onClick={() => void trackOnlineFixDownload(result, true, link.url)}
                                title="Open this exact Hosters/Drive source in your default browser if its controls do not work inside Dusk">
                                <ExternalLink size={12} /> Browser fallback
                              </button>
                            </div>
                          ))}
                          {!downloadSources[result.url].some((link) => link.kind === "game" || link.kind === "mirror") && (
                            <span>No verified full-game download sources found.</span>
                          )}
                        </div>
                      )}
                    </div>
                    ) : (
                      <div className="web-result-actions">
                        <button className="button primary" disabled={downloadSourcesBusy !== null}
                          onClick={() => void oneClickGameSourceDownload(result)}
                          title="Find direct game archives, download all required volumes, and try alternate mirrors">
                          {downloadSourcesBusy === result.url ? <RefreshCw className="spin" size={15} /> : <Archive size={15} />}
                          {downloadSourcesBusy === result.url ? "Finding sources…" : "Get game"}
                        </button>
                        <button className="button secondary" onClick={() => void openOtherGameSource(result)}
                          title="Open inside Dusk for hosts that require login, a captcha, or an extra download step">
                          <Globe2 size={15} /> Open in Dusk
                        </button>
                        <button className="button ghost" onClick={() => void openOtherGameSource(result, true)}
                          title="Use system browser when needed; Dusk watches for a matching downloaded archive">
                          <ExternalLink size={15} /> Browser fallback
                        </button>
                      </div>
                    )}
                  </article>
                ))}
              </div>
              <p className="web-results-note">
                {webSource === "online-fix"
                  ? "Get game starts supported downloads and safely imports completed single-file archives in the background. Home and Library show games, not download forms. Login-only hosts still need browser interaction."
                  : webSource === "fitgirl"
                    ? "Get game downloads discoverable direct archives, including multiple volumes and alternate mirrors. Hosts that require login, captchas or special steps still need Open in Dusk."
                    : "Get game tries direct archive links with alternate mirrors and multipart support. Browser-based hosts still need manual steps; embedded browsing keeps ad blocking on."}
              </p>
            </section>
          ) : (
            <>
              {view === "trainerCatalog" && (
                <TrainerCatalogBrowser
                  state={trainerCatalogState}
                  results={trainerCatalogEntries}
                  query={trainerCatalogQuery}
                  statusFilter={trainerCatalogStatusFilter}
                  page={trainerCatalogPage}
                  errorMessage={trainerCatalogError}
                  onlineTableState={catalogOnlineTableState}
                  onlineTableResults={catalogOnlineTableResults}
                  onlineTableError={catalogOnlineTableError}
                  onlineTableBusyId={catalogOnlineTableBusyId}
                  onSelectGame={openCatalogGame}
                  onSearchOnlineTables={(query) => void searchCatalogOnlineTables(query)}
                  onSearchNexusMods={(query) => void searchNexusMods(query)}
                  onImportOnlineTable={(result) => void importCatalogOnlineTable(result)}
                  onQueryChange={(nextQuery) => {
                    setTrainerCatalogQuery(nextQuery);
                    setTrainerCatalogPage(1);
                  }}
                  onStatusFilterChange={(nextFilter) => {
                    setTrainerCatalogStatusFilter(nextFilter);
                    setTrainerCatalogPage(1);
                  }}
                  onRetry={() => void loadTrainerCatalog(true).catch(() => {})}
                  onPageChange={setTrainerCatalogPage}
                />
              )}

              {view === "home" && (
                <div className="page">
                  <section className="hero home-hero">
                    <div className="home-hero-main">
                      {recentGames[0]?.coverDataUrl && (
                        <img className="home-hero-art" src={recentGames[0].coverDataUrl} alt="" />
                      )}
                      {!recentGames[0]?.coverDataUrl && recentGames[0]?.coverPath && (
                        <img className="home-hero-art" src={convertFileSrc(recentGames[0].coverPath)} alt="" />
                      )}
                      <div className="home-hero-copy">
                        <span className="eyebrow">{recentGames[0] ? "Continue playing" : "Local library"}</span>
                        <h1>{recentGames[0]?.title || "Your games. One place."}</h1>
                        <p>
                          {recentGames[0]
                            ? `${formatDuration(recentGames[0].totalSeconds)} tracked · ${recentGames[0].launchCount} sessions`
                            : "Your installed games, playtime, saves, and screenshots stay together in Dusk."}
                        </p>
                        <div className="hero-actions">
                          {recentGames[0] ? (
                            <>
                              <button className="button primary" onClick={() => void play(recentGames[0])} disabled={playingId === recentGames[0].id}>
                                <Play size={16} />
                                {playingId === recentGames[0].id ? "Launching…" : "Play"}
                              </button>
                              <button className="button secondary" onClick={() => setSelectedGameId(recentGames[0].id)}>
                                Game details
                              </button>
                            </>
                          ) : (
                            <>
                              <button className="button primary" onClick={() => void scan()} disabled={scanning}>
                                <ScanSearch size={16} />
                                {scanning ? "Scanning…" : "Scan this PC"}
                              </button>
                              <button className="button secondary" onClick={() => setAddOpen(true)}>
                                <Plus size={16} />
                                Add manually
                              </button>
                            </>
                          )}
                          <button
                            className="button ghost"
                            onClick={() => {
                              setView("trainerCatalog");
                              void loadTrainerCatalog().catch(() => {});
                            }}
                          >
                            Browse Dusk Catalog
                          </button>
                        </div>
                      </div>
                    </div>
                    <div className="hero-stat">
                      <span>Total playtime</span>
                      <strong>{formatDuration(stats.totalSeconds)}</strong>
                      <small>
                        {stats.topGame ? "Most played: " + stats.topGame : "No tracked sessions yet"}
                      </small>
                    </div>
                  </section>

                  <section className="stat-grid">
                    <div className="stat-card">
                      <Library size={18} />
                      <span>Games</span>
                      <strong>{stats.gameCount}</strong>
                    </div>
                    <div className="stat-card">
                      <Clock3 size={18} />
                      <span>Last 7 days</span>
                      <strong>{formatDuration(stats.last7DaysSeconds)}</strong>
                    </div>
                    <div className="stat-card">
                      <Play size={18} />
                      <span>Tracked sessions</span>
                      <strong>{stats.launchCount}</strong>
                    </div>
                    <div className="stat-card">
                      <Images size={18} />
                      <span>Screenshots</span>
                      <strong>{stats.screenshotCount}</strong>
                    </div>
                  </section>

                  <section className="content-section">
                    <div className="section-heading">
                      <div>
                        <span className="eyebrow">Continue</span>
                        <h2>Recently played</h2>
                      </div>
                      <button
                        className="text-action"
                        onClick={() => {
                          setSortMode("recent");
                          setView("library");
                        }}
                      >
                        See library
                      </button>
                    </div>

                    {recentGames.length === 0 ? (
                      <EmptyState
                        icon={<Gamepad2 size={24} />}
                        title="No tracked sessions yet"
                        copy="Scan your PC or add a game manually, then launch it through Dusk."
                        action={
                          <button className="button primary" onClick={() => void scan()}>
                            Scan for games
                          </button>
                        }
                      />
                    ) : (
                      <div className="game-grid">
                        {recentGames.map((game) => (
                          <GameCard
                            key={game.id}
                            game={game}
                            playing={playingId === game.id}
                            onOpen={() => setSelectedGameId(game.id)}
                            onPlay={() => void play(game)}
                            onFavorite={() => void toggleFavorite(game)}
                          />
                        ))}
                      </div>
                    )}
                  </section>

                  {favoriteGames.length > 0 && (
                    <section className="content-section">
                      <div className="section-heading">
                        <div>
                          <span className="eyebrow">Pinned</span>
                          <h2>Favorites</h2>
                        </div>
                      </div>
                      <div className="game-grid">
                        {favoriteGames.map((game) => (
                          <GameCard
                            key={game.id}
                            game={game}
                            playing={playingId === game.id}
                            onOpen={() => setSelectedGameId(game.id)}
                            onPlay={() => void play(game)}
                            onFavorite={() => void toggleFavorite(game)}
                          />
                        ))}
                      </div>
                    </section>
                  )}
                </div>
              )}

              {(view === "library" || view === "favorites") && (
                <div className="page">
                  <div className="page-heading">
                    <div>
                      <span className="eyebrow">
                        {collectionFilter === "all" ? "Games" : "Collection"}
                      </span>
                      <h1>{libraryTitle}</h1>
                      <p>
                        {visibleGames.length} {visibleGames.length === 1 ? "game" : "games"} shown
                      </p>
                    </div>
                    <div className="library-controls">
                      <select
                        value={sourceFilter}
                        onChange={(event) => setSourceFilter(event.target.value)}
                        aria-label="Filter by source"
                      >
                        <option value="all">All sources</option>
                        <option value="steam">Steam</option>
                        <option value="epic">Epic</option>
                        <option value="gog">GOG</option>
                        <option value="emulator">Emulators</option>
                        <option value="device">Device scan</option>
                        <option value="manual">Manual</option>
                      </select>
                      <select
                        value={sortMode}
                        onChange={(event) => setSortMode(event.target.value as SortMode)}
                        aria-label="Sort games"
                      >
                        <option value="name">Name</option>
                        <option value="recent">Recently played</option>
                        <option value="playtime">Playtime</option>
                      </select>
                    </div>
                  </div>

                  {visibleGames.length === 0 ? (
                    <EmptyState
                      icon={view === "favorites" ? <Heart size={25} /> : <Library size={25} />}
                      title={games.length === 0 ? "Your library is empty" : "No games match these filters"}
                      copy={
                        games.length === 0
                          ? "Dusk automatically detects Steam, Epic, GOG, common emulators, and games in common device game folders."
                          : "Change the search, source, or collection filter."
                      }
                      action={
                        games.length === 0 ? (
                          <div className="empty-actions">
                            <button className="button primary" onClick={() => void scan()}>
                              <ScanSearch size={16} />
                              Scan PC
                            </button>
                            <button className="button secondary" onClick={() => setAddOpen(true)}>
                              <Plus size={16} />
                              Add game
                            </button>
                          </div>
                        ) : undefined
                      }
                    />
                  ) : (
                    <div className="game-grid large">
                      {visibleGames.map((game) => (
                        <GameCard
                          key={game.id}
                          game={game}
                          playing={playingId === game.id}
                          onOpen={() => setSelectedGameId(game.id)}
                          onPlay={() => void play(game)}
                          onFavorite={() => void toggleFavorite(game)}
                        />
                      ))}
                    </div>
                  )}
                </div>
              )}

              {view === "screenshots" && (
                <div className="page">
                  <div className="page-heading">
                    <div>
                      <span className="eyebrow">Local gallery</span>
                      <h1>Screenshots</h1>
                      <p>Automatically detected and manually imported local screenshots.</p>
                    </div>
                    <div className="top-actions">
                      <button
                        className="button primary"
                        disabled={screenshotScanning}
                        onClick={() => void scanScreenshotsNow()}
                      >
                        {screenshotScanning ? (
                          <RefreshCw className="spin" size={16} />
                        ) : (
                          <ScanSearch size={16} />
                        )}
                        {screenshotScanning ? "Scanning…" : "Scan screenshots"}
                      </button>
                      <button className="button secondary" onClick={() => void refreshScreenshots()}>
                        <RefreshCw size={16} />
                        Refresh
                      </button>
                    </div>
                  </div>

                  {screenshots.length === 0 ? (
                    <EmptyState
                      icon={<Images size={25} />}
                      title="No screenshots found"
                      copy="Dusk checks Steam screenshot folders, common per-game screenshot folders, Windows Screenshots, and Xbox Game Bar Captures. You can also import images manually from a game."
                    />
                  ) : (
                    <div className="screenshot-grid">
                      {screenshots.map((screenshot) => {
                        const game = games.find((item) => item.id === screenshot.gameId);
                        return (
                          <article className="screenshot-card" key={screenshot.id}>
                            <img
                              src={convertFileSrc(screenshot.path)}
                              alt=""
                              loading="lazy"
                              decoding="async"
                              draggable={false}
                            />
                            <div className="screenshot-meta">
                              <div>
                                <strong>{game?.title || "Unknown game"}</strong>
                                <span>{formatDate(screenshot.createdAt)}</span>
                              </div>
                              <button
                                className="icon-button"
                                onClick={() => void deleteScreenshot(screenshot)}
                                title="Delete imported screenshot"
                              >
                                <Trash2 size={16} />
                              </button>
                            </div>
                          </article>
                        );
                      })}
                    </div>
                  )}
                </div>
              )}


              {view === "achievements" && (
                <div className="page">
                  <div className="page-heading">
                    <div>
                      <span className="eyebrow">Dusk milestones</span>
                      <h1>Achievements</h1>
                      <p>
                        Launcher milestones calculated from real Dusk library and session data,
                        not achievements imported from game platforms.
                      </p>
                    </div>
                  </div>

                  <div className="achievement-grid">
                    {achievements.map((achievement) => {
                      const percent = Math.min(
                        100,
                        Math.round((achievement.current / achievement.target) * 100),
                      );
                      return (
                        <article
                          key={achievement.id}
                          className={cx("achievement-card", achievement.unlocked && "unlocked")}
                        >
                          <div className="achievement-icon">
                            {achievement.unlocked ? <Trophy size={21} /> : <Sparkles size={21} />}
                          </div>
                          <div>
                            <div className="achievement-title">
                              <h3>{achievement.title}</h3>
                              {achievement.unlocked && <span>Unlocked</span>}
                            </div>
                            <p>{achievement.description}</p>
                            <div className="progress-track">
                              <div style={{ width: String(percent) + "%" }} />
                            </div>
                            <small>
                              {achievement.current} / {achievement.target}
                            </small>
                          </div>
                        </article>
                      );
                    })}
                  </div>
                </div>
              )}

              {view === "settings" && (
                <div className="page settings-page">
                  <div className="page-heading">
                    <div>
                      <span className="eyebrow">Dusk</span>
                      <h1>Settings</h1>
                      <p>Account, profiles, appearance, updates, and local data.</p>
                    </div>
                  </div>

                  <div className="settings-layout">
                    <nav className="settings-nav" aria-label="Settings sections">
                      <span className="settings-nav-heading">Preferences</span>
                      {!guestMode && (
                        <button type="button" className="settings-nav-item" onClick={() => scrollToSettingsSection("settings-account")}>
                          <UserRound size={15} /> Account
                        </button>
                      )}
                      <button type="button" className="settings-nav-item" onClick={() => scrollToSettingsSection("settings-profiles")}>
                        <Layers3 size={15} /> Profiles &amp; saves
                      </button>
                      <button type="button" className="settings-nav-item" onClick={() => scrollToSettingsSection("settings-appearance")}>
                        <Moon size={15} /> Appearance
                      </button>
                      <button type="button" className="settings-nav-item" onClick={() => scrollToSettingsSection("settings-detection")}>
                        <ScanSearch size={15} /> Game detection
                      </button>
                      <button type="button" className="settings-nav-item" onClick={() => scrollToSettingsSection("settings-installers")}>
                        <PackageOpen size={15} /> Installers
                      </button>
                      <button type="button" className="settings-nav-item" onClick={() => scrollToSettingsSection("settings-updates")}>
                        <RefreshCw size={15} /> Updates
                      </button>
                      <button type="button" className="settings-nav-item" onClick={() => scrollToSettingsSection("settings-data")}>
                        <Database size={15} /> Local data
                      </button>
                    </nav>

                    <div className="settings-content">
                  {!guestMode && (
                    <div id="settings-account" className="settings-section-anchor">
                      <AccountProfileSettings onToast={showToast} />
                    </div>
                  )}

                  <section id="settings-profiles" className="settings-card">
                    <div className="settings-card-head">
                      <UserRound size={20} />
                      <div>
                        <h3>Profiles & cloud saves</h3>
                        <p>Separate game libraries and save backups for each owner.</p>
                      </div>
                    </div>

                    <div className="profile-settings-list">
                      {profiles.map((profile) => (
                        <div className={cx("profile-settings-row", activeProfile?.id === profile.id && "active")} key={profile.id}>
                          <div>
                            <strong>{profile.name}</strong>
                            <span>
                              {profile.gameCount} games · {profile.backupCount} backups
                              {activeProfile?.id === profile.id ? " · active" : ""}
                            </span>
                          </div>
                          <div className="profile-settings-actions">
                            {activeProfile?.id !== profile.id && (
                              <button className="button secondary" onClick={() => void switchOwnerProfile(profile.id)}>
                                Use
                              </button>
                            )}
                            <button className="icon-button" onClick={() => void renameOwnerProfile(profile)} title="Rename profile">
                              <Pencil size={15} />
                            </button>
                            {profiles.length > 1 && (
                              <button className="icon-button" onClick={() => void removeOwnerProfile(profile)} title="Delete profile">
                                <Trash2 size={15} />
                              </button>
                            )}
                          </div>
                        </div>
                      ))}
                      <button className="button secondary" onClick={() => void createOwnerProfile()}>
                        <Plus size={15} />
                        New owner profile
                      </button>
                    </div>

                    <div className="setting-row">
                      <div>
                        <strong>{guestMode ? "Guest mode" : "Dusk account"}</strong>
                        <span>
                          {guestMode
                            ? "Your library and saves stay local to this PC. Sign in anytime to enable cross-device sync."
                            : "Your account owns this synced library, progress, profiles, collections, and cloud-save namespace."}
                        </span>
                      </div>
                      <button
                        className="button secondary"
                        onClick={() => {
                          localStorage.removeItem("dusk-account-mode");
                          void api
                            .setAccountScope(null)
                            .then(() => (guestMode ? undefined : logoutDuskAccount()))
                            .then(() => window.location.reload());
                        }}
                      >
                        {guestMode ? "Sign in" : "Sign out"}
                      </button>
                    </div>
                    <div className="setting-row cloud-save-row">
                      <div>
                        <strong>Supabase save files</strong>
                        <span>{cloudStatus.message}</span>
                      </div>
                      <div className={cx("cloud-status-pill", cloudStatus.authenticated && "connected")}>
                        <Cloud size={14} />
                        {cloudStatus.authenticated ? "Connected" : cloudStatus.configured ? "Needs auth" : "Not configured"}
                      </div>
                    </div>
                    <div className="setting-row">
                      <div>
                        <strong>Sync every owner</strong>
                        <span>
                          Upload all existing save backup files for every profile/game to the private Supabase bucket.
                        </span>
                      </div>
                      <button
                        className="button primary"
                        disabled={cloudSyncBusy || !cloudStatus.authenticated}
                        onClick={() => void syncEveryOwnerToCloud()}
                      >
                        <Cloud size={15} />
                        {cloudSyncBusy ? "Syncing…" : "Sync all saves"}
                      </button>
                    </div>
                  </section>

                  <section id="settings-console" className="settings-card">
                    <div className="settings-card-head">
                      <Gamepad2 size={20} />
                      <div>
                        <h3>Console mode</h3>
                        <p>Fullscreen layout designed for controller navigation.</p>
                      </div>
                    </div>

                    <div className="setting-row">
                      <div>
                        <strong>{controllerConnected ? "Controller connected" : "Controller navigation"}</strong>
                        <span>
                          D-pad or left stick moves focus · A selects · X launches the focused game ·
                          B goes back · Menu toggles fullscreen.
                        </span>
                      </div>
                      <button
                        className={cx("button", consoleMode ? "primary" : "secondary")}
                        onClick={() => void setConsoleModeEnabled(!consoleMode)}
                      >
                        <Gamepad2 size={16} />
                        {consoleMode ? "Exit console mode" : "Enter console mode"}
                      </button>
                    </div>
                  </section>

                  <section id="settings-appearance" className="settings-card">
                    <div className="settings-card-head">
                      <Moon size={20} />
                      <div>
                        <h3>Appearance</h3>
                        <p>Stored locally in this app.</p>
                      </div>
                    </div>

                    <div className="setting-row">
                      <div>
                        <strong>Theme</strong>
                        <span>Choose the base surface style.</span>
                      </div>
                      <div className="segmented">
                        {(["night", "oled", "slate"] as ThemeName[]).map((item) => (
                          <button
                            key={item}
                            className={theme === item ? "active" : ""}
                            onClick={() => setTheme(item)}
                          >
                            {item}
                          </button>
                        ))}
                      </div>
                    </div>

                    <div className="setting-row">
                      <div>
                        <strong>Accent</strong>
                        <span>Used for primary actions and highlights.</span>
                      </div>
                      <div className="accent-options">
                        {(["violet", "ember", "cyan"] as AccentName[]).map((item) => (
                          <button
                            key={item}
                            className={cx("accent-swatch", item, accent === item && "active")}
                            onClick={() => setAccent(item)}
                            aria-label={item + " accent"}
                          >
                            {accent === item && <Check size={14} />}
                          </button>
                        ))}
                      </div>
                    </div>
                  </section>

                  <section id="settings-presence" className="settings-card">
                    <div className="settings-card-head">
                      <Radio size={20} />
                      <div>
                        <h3>Discord Rich Presence</h3>
                        <p>Optional launcher presence. Disabled by default.</p>
                      </div>
                    </div>

                    <div className="setting-row">
                      <div>
                        <strong>{discordRpcEnabled ? "Connected as Dusk" : "No Dusk RPC activity"}</strong>
                        <span>
                          Dusk cannot control Discord's separate Registered Games scanner. This optional
                          RPC gives Dusk an intentional launcher identity instead of relying on automatic
                          process detection.
                        </span>
                      </div>
                      <button
                        className={cx("button", discordRpcEnabled ? "primary" : "secondary")}
                        onClick={() => void (discordRpcEnabled ? disableDiscordRpc() : enableDiscordRpc())}
                      >
                        <Radio size={15} />
                        {discordRpcEnabled ? "Disable" : "Enable"}
                      </button>
                    </div>

                    <div className="discord-rpc-config">
                      <label>
                        <span>Discord Application ID</span>
                        <input
                          value={discordClientId}
                          onChange={(event) =>
                            setDiscordClientId(event.target.value.replace(/\D/g, "").slice(0, 24))
                          }
                          inputMode="numeric"
                          placeholder="Create a Dusk app in Discord Developer Portal"
                          disabled={discordRpcEnabled}
                        />
                      </label>
                      <p>
                        Use a Discord application named <strong>Dusk</strong>. Rich Presence stays off
                        until you enable it here.
                      </p>
                    </div>
                  </section>

                  <section id="settings-updates" className="settings-card">
                    <div className="settings-card-head">
                      <RefreshCw size={20} />
                      <div>
                        <h3>Updates</h3>
                        <p>Secure updates from official Dusk GitHub Releases.</p>
                      </div>
                    </div>
                    <div className="setting-row">
                      <div>
                        <strong>
                          {availableUpdate ? "Dusk " + availableUpdate + " available" : "Automatic updater"}
                        </strong>
                        <span>
                          {availableUpdate
                            ? "Restart Dusk once; it updates silently and opens the new version automatically."
                            : "Dusk checks for newer releases automatically. You can also check now."}
                        </span>
                      </div>
                      {availableUpdate ? (
                        <button
                          className="button primary"
                          disabled={updateBusy}
                          onClick={() => void installUpdate()}
                        >
                          {updateBusy ? "Preparing update…" : "Restart to update"}
                        </button>
                      ) : (
                        <button
                          className="button secondary"
                          disabled={updateBusy}
                          onClick={() => void checkUpdates()}
                        >
                          {updateBusy ? "Checking…" : "Check for updates"}
                        </button>
                      )}
                    </div>
                    {availableUpdate && availableUpdateNotes && (
                      <div className="update-release-notes" role="status" aria-label="Available update changelog">
                        <strong>What's new in Dusk {availableUpdate}</strong>
                        <div className="update-release-notes-text">{availableUpdateNotes}</div>
                      </div>
                    )}
                  </section>

                  <section id="settings-data" className="settings-card">
                    <div className="settings-card-head">
                      <Database size={20} />
                      <div>
                        <h3>Local data</h3>
                        <p>Dusk does not require a cloud account.</p>
                      </div>
                    </div>
                    <div className="data-path">
                      <span>Application data directory</span>
                      <code>{dataDirectory || "Unavailable"}</code>
                    </div>
                    <div className="settings-note">
                      <HardDrive size={17} />
                      <p>
                        Playtime, collections, imported screenshot references, and Dusk settings
                        stay on this PC. Dusk only records a session when it can observe the launched
                        process.
                      </p>
                    </div>
                  </section>

                  <section id="settings-installers" className="settings-card">
                    <div className="settings-card-head">
                      <PackageOpen size={20} />
                      <div>
                        <h3>Installer hub</h3>
                        <p>Install from local files or open supported official stores.</p>
                      </div>
                    </div>

                    <div className="setting-row">
                      <div>
                        <strong>Run local game installer</strong>
                        <span>
                          Choose a local .exe or .msi you already have. Dusk launches it, then
                          automatic scanning can discover the installed game.
                        </span>
                      </div>
                      <button
                        className="button primary"
                        disabled={installerBusy}
                        onClick={() => void runLocalInstaller()}
                      >
                        <PackageOpen size={16} />
                        {installerBusy ? "Opening…" : "Choose installer"}
                      </button>
                    </div>

                    <div className="official-store-grid">
                      <button className="button secondary" onClick={() => void openExternal("steam")}>
                        Steam <ExternalLink size={14} />
                      </button>
                      <button className="button secondary" onClick={() => void openExternal("epic")}>
                        Epic Games <ExternalLink size={14} />
                      </button>
                      <button className="button secondary" onClick={() => void openExternal("gog")}>
                        GOG <ExternalLink size={14} />
                      </button>
                      <button className="button secondary" onClick={() => void openExternal("itch")}>
                        itch.io <ExternalLink size={14} />
                      </button>
                    </div>

                    <div className="settings-note">
                      <HardDrive size={17} />
                      <p>
                        Dusk does not download games from unofficial redistribution sites. Local
                        installers stay under your control, and official store links open in your
                        default browser.
                      </p>
                    </div>
                  </section>

                  <section id="settings-detection" className="settings-card">
                    <div className="settings-card-head">
                      <ScanSearch size={20} />
                      <div>
                        <h3>Game detection</h3>
                        <p>Current automatic scanners.</p>
                      </div>
                    </div>
                    <div className="setting-row">
                      <div>
                        <strong>Automatic device scan</strong>
                        <span>
                          Runs shortly after startup and about every 10 minutes while Dusk is open.
                          Scans are bounded and run off the UI thread.
                          {lastAutoScanAt
                            ? " Last scan: " + formatDate(new Date(lastAutoScanAt).toISOString()) + "."
                            : ""}
                        </span>
                      </div>
                      <button
                        className={cx("button", autoScanEnabled ? "primary" : "secondary")}
                        onClick={() => {
                          const next = !autoScanEnabled;
                          setAutoScanEnabled(next);
                          localStorage.setItem("dusk-auto-scan", String(next));
                          if (next) void autoScanGames(true);
                        }}
                      >
                        {autoScanEnabled ? "On" : "Off"}
                      </button>
                    </div>

                    <div className="scanner-list">
                      <div>
                        <Check size={16} />
                        <strong>Steam</strong>
                        <span>Reads local app manifests and library folders.</span>
                      </div>
                      <div>
                        <Check size={16} />
                        <strong>Epic Games</strong>
                        <span>Reads installed Epic manifest files.</span>
                      </div>
                      <div>
                        <Check size={16} />
                        <strong>GOG</strong>
                        <span>Reads GOG game install records from the Windows registry.</span>
                      </div>
                      <div>
                        <Check size={16} />
                        <strong>Emulators</strong>
                        <span>Detects common installed emulators such as Dolphin, PCSX2, RetroArch, Ryujinx, Cemu, PPSSPP, and DuckStation.</span>
                      </div>
                      <div>
                        <Check size={16} />
                        <strong>Device folders</strong>
                        <span>Checks bounded common game folders across Windows drives, your profile, and Downloads without crawling the whole disk.</span>
                      </div>
                      <div>
                        <Check size={16} />
                        <strong>Downloads</strong>
                        <span>Scans downloaded game folders by default while ignoring loose installer executables.</span>
                      </div>
                      <div>
                        <Plus size={16} />
                        <strong>Manual games</strong>
                        <span>Any local Windows executable can be added explicitly.</span>
                      </div>
                    </div>
                  </section>

                  <section className="about-card">
                    <div className="brand-mark">
                      <img className="brand-logo-image" src={duskLogo} alt="" draggable={false} />
                    </div>
                    <div>
                      <strong>Dusk{appVersion ? ` ${appVersion}` : ""} · bxane</strong>
                      <span>Open code · github.com/bxanedot/dusk</span>
                      <button className="about-creator-pill" onClick={() => void openExternal("creator")}>
                        bxane · guns.lol/bxane <ExternalLink size={11} />
                      </button>
                    </div>
                  </section>
                    </div>
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </main>

      {addOpen && (
        <AddGameModal
          onClose={() => setAddOpen(false)}
          onAdded={() => refreshCore(false)}
        />
      )}

      {selectedGame && (
        <GameDetail
          game={selectedGame}
          activeProfile={activeProfile}
          collections={collections}
          memberships={memberships}
          onClose={() => setSelectedGameId(null)}
          onPlay={play}
          onRefresh={() => refreshCore(false)}
          onToast={showToast}
          onSearchCatalog={searchTrainerCatalog}
          onBrowseCatalog={() => {
            setView("trainerCatalog");
            setWebSearched(false);
            void loadTrainerCatalog().catch(() => {});
          }}
        />
      )}

      {controllerConnected && (
        <div className="controller-bar" aria-live="polite">
          <span><kbd>A</kbd> Select</span>
          <span><kbd>X</kbd> Play</span>
          <span><kbd>B</kbd> Back</span>
          <span><kbd>Menu</kbd> {consoleMode ? "Exit fullscreen" : "Fullscreen"}</span>
        </div>
      )}

      {toast && <div className={cx("toast", toast.type)}>{toast.message}</div>}
      </div>
    </div>
  );
}
