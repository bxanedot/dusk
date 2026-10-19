import { Download, FolderOpen, FolderPlus, PackageCheck, Plus, Search, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import { NameEditDialog } from "./NameEditDialog";
import type { GameRecord, ModManagerOverview, ThunderstoreModResult, ThunderstoreSearchResult } from "../types";

function formatBytes(bytes: number) {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const unit = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** unit).toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

export function ModManagerPanel(props: {
  game: GameRecord;
  onToast: (message: string, type?: "ok" | "error") => void;
}) {
  const [state, setState] = useState<ModManagerOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [catalogBusy, setCatalogBusy] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [catalogQuery, setCatalogQuery] = useState("");
  const [catalog, setCatalog] = useState<ThunderstoreSearchResult | null>(null);
  const [newProfileOpen, setNewProfileOpen] = useState(false);
  const requestLock = useRef(false);

  useEffect(() => {
    let current = true;
    setLoading(true);
    setCatalog(null);
    setCatalogQuery("");
    api.listModManager(props.game.id)
      .then((value) => { if (current) setState(value); })
      .catch((error) => { if (current) props.onToast(String(error), "error"); })
      .finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [props.game.id]);

  async function run(action: () => Promise<ModManagerOverview>, message: string) {
    if (requestLock.current) return;
    requestLock.current = true;
    setBusy(true);
    try {
      setState(await action());
      props.onToast(message);
    } catch (error) {
      try { setState(await api.listModManager(props.game.id)); } catch { /* Keep the current state if refresh also fails. */ }
      props.onToast(String(error), "error");
    } finally {
      requestLock.current = false;
      setBusy(false);
    }
  }

  async function createProfile(name: string) {
    if (requestLock.current) throw new Error("A mod change is already in progress.");
    requestLock.current = true;
    setBusy(true);
    try {
      setState(await api.createModProfile(props.game.id, name));
      props.onToast(`Created ${name}.`);
    } catch (error) {
      try { setState(await api.listModManager(props.game.id)); } catch { /* Keep the current profile state if refresh also fails. */ }
      throw error;
    } finally {
      requestLock.current = false;
      setBusy(false);
    }
  }

  async function importFolder() {
    if (requestLock.current) return;
    requestLock.current = true;
    setBusy(true);
    try {
      const next = await api.importModFolder(props.game.id);
      if (next) {
        setState(next);
        props.onToast("Mod copied into Dusk's library.");
      }
    } catch (error) {
      props.onToast(String(error), "error");
    } finally {
      requestLock.current = false;
      setBusy(false);
    }
  }

  async function searchCatalog() {
    if (requestLock.current) return;
    requestLock.current = true;
    setCatalogBusy(true);
    try {
      setCatalog(await api.searchThunderstoreMods(props.game.id, catalogQuery));
    } catch (error) {
      props.onToast(String(error), "error");
    } finally {
      requestLock.current = false;
      setCatalogBusy(false);
    }
  }

  async function installCatalogMod(mod: ThunderstoreModResult) {
    const packageId = mod.namespace + "-" + mod.name;
    const dependencies = mod.dependencyCount ? " and " + mod.dependencyCount + " declared dependency package(s)" : "";
    const prompt = "Download and enable " + packageId + " " + mod.version + dependencies + " for " + props.game.title + "? Package files will be placed in the game folder and loaded by its mod loader. Dusk backs up replaced files for safe removal.\n\nTarget: " + (state?.gamePath ?? "");
    if (!window.confirm(prompt) || requestLock.current) return;
    requestLock.current = true;
    setInstalling(packageId);
    try {
      setState(await api.installThunderstoreMod(props.game.id, mod));
      props.onToast("Installed " + packageId + " " + mod.version + " and enabled its dependencies.");
    } catch (error) {
      try { setState(await api.listModManager(props.game.id)); } catch { /* Preserve the current profile if refresh also fails. */ }
      props.onToast(String(error), "error");
    } finally {
      requestLock.current = false;
      setInstalling(null);
    }
  }

  async function toggleMod(id: string, name: string, enabled: boolean) {
    if (!state || requestLock.current) return;
    if (enabled && !window.confirm(
      `Deploy “${name}” into ${props.game.title}? Dusk will back up any replaced files and restore them when disabled. Files changed after deployment will be left untouched.\n\nTarget: ${state.gamePath}`,
    )) return;
    requestLock.current = true;
    setBusy(true);
    try {
      const result = await api.setModEnabled(props.game.id, id, enabled);
      setState(await api.listModManager(props.game.id));
      if (result.conflicts.length) {
        props.onToast(`Dusk left ${result.conflicts.length} changed game file(s) untouched. Restore them before disabling this mod.`, "error");
      } else {
        props.onToast(enabled ? `Deployed ${name}.` : `Disabled ${name} and restored its files.`);
      }
    } catch (error) {
      props.onToast(String(error), "error");
    } finally {
      requestLock.current = false;
      setBusy(false);
    }
  }

  async function removeMod(id: string, name: string, enabled: boolean) {
    if (!window.confirm(
      `Remove “${name}” from this profile and delete Dusk's stored copy? The original folder you imported stays untouched.${enabled ? " Dusk will restore files it can verify first." : ""}`,
    )) return;
    await run(() => api.removeMod(props.game.id, id), `Removed ${name} from this profile.`);
  }

  const activeProfile = state?.profiles.find((profile) => profile.id === state.activeProfileId);

  return (
    <div className="mod-manager">
      <div className="detail-section-heading mod-manager-heading">
        <div>
          <h3><PackageCheck size={16} /> Mods</h3>
          <p>Browse, install, and manage game mods with reversible deployment.</p>
        </div>
        <button className="button secondary mod-import-button" disabled={busy || loading} onClick={() => void importFolder()}>
          <FolderPlus size={15} /> Import folder
        </button>
      </div>

      {loading ? <p className="muted">Loading mod profiles…</p> : state ? <>
        <div className="mod-profile-toolbar">
          <label htmlFor={`mod-profile-${props.game.id}`}>Profile</label>
          <select
            id={`mod-profile-${props.game.id}`}
            value={state.activeProfileId}
            disabled={busy}
            onChange={(event) => {
              const nextProfile = state.profiles.find((profile) => profile.id === event.currentTarget.value);
              if (!nextProfile) return;
              if (!window.confirm(
                `Switch to “${nextProfile.name}”? Dusk will safely restore the current profile's deployed files and apply the selected profile's enabled mods.`,
              )) {
                event.currentTarget.value = state.activeProfileId;
                return;
              }
              void run(
                () => api.setActiveModProfile(props.game.id, nextProfile.id),
                `Switched to ${nextProfile.name}.`,
              );
            }}
          >
            {state.profiles.map((profile) => <option key={profile.id} value={profile.id}>
              {profile.name} · {profile.enabledCount} active
            </option>)}
          </select>
          <button
            className="icon-button"
            title="Create mod profile"
            aria-label="Create mod profile"
            disabled={busy}
            onClick={() => setNewProfileOpen(true)}
          ><Plus size={15} /></button>
        </div>

        {state.profiles.filter((profile) => !profile.active).length > 0 && <div className="mod-other-profiles">
          {state.profiles.filter((profile) => !profile.active).map((profile) => <div className="mod-other-profile" key={profile.id}>
            <span>{profile.name} · {profile.modCount} mods · {profile.enabledCount} enabled</span>
            <button className="icon-button danger" title={`Remove ${profile.name} profile`}
              aria-label={`Remove ${profile.name} profile`} disabled={busy}
              onClick={() => {
                if (window.confirm(`Remove the “${profile.name}” profile and all mod copies stored in it?`)) {
                  void run(
                    () => api.removeModProfile(props.game.id, profile.id),
                    `Removed ${profile.name}.`,
                  );
                }
              }}><Trash2 size={14} /></button>
          </div>)}
        </div>}

        <p className="mod-manager-note">
          Import an extracted folder or install a Thunderstore package. Dusk stores a separate copy, backs up replaced files, and only restores files that have not changed since deployment. If a game move leaves both old and new install folders, resolve the duplicate before managing mods.
        </p>
        <div className="mod-target-path" title={state.gamePath}>{state.gamePath}</div>

        <div className="mod-catalog">
          <div className="mod-catalog-heading">
            <div>
              <strong>Thunderstore</strong>
              <span>Game specific packages, loader rules, and dependencies</span>
            </div>
            {catalog && <span className="mod-catalog-loader">{catalog.gameName} · {catalog.loader}</span>}
          </div>
          <form className="mod-catalog-search" onSubmit={(event) => { event.preventDefault(); void searchCatalog(); }}>
            <label className="sr-only" htmlFor={"mod-search-" + props.game.id}>Search Thunderstore mods</label>
            <input
              id={"mod-search-" + props.game.id}
              type="search"
              value={catalogQuery}
              onChange={(event) => setCatalogQuery(event.currentTarget.value)}
              placeholder="Search packages or leave blank for popular"
              disabled={catalogBusy || busy}
            />
            <button className="button secondary" type="submit" disabled={catalogBusy || busy}>
              <Search size={14} /> {catalogBusy ? "Searching…" : "Search"}
            </button>
          </form>
          <p className="mod-catalog-hint">Declared dependencies install first. Runtime plugin order is controlled by the game loader.</p>
          {catalogBusy ? <p className="muted" role="status">Loading the community catalog…</p> : catalog ? <>
            <div className="mod-catalog-results-heading" aria-live="polite">
              {catalog.mods.length} packages · sorted by pinned, rating, then downloads
            </div>
            {catalog.mods.length ? <div className="mod-list mod-catalog-list">
              {catalog.mods.map((mod) => {
                const packageId = mod.namespace + "-" + mod.name;
                const isInstalling = installing === packageId;
                return <article className="mod-row mod-catalog-row" key={packageId + "-" + mod.version}>
                  <div className="mod-row-copy">
                    <strong>{mod.namespace} / {mod.name}</strong>
                    <span>{mod.version} · {mod.downloads.toLocaleString()} downloads · {mod.dependencyCount} dependencies{mod.pinned ? " · Pinned" : ""}</span>
                    {mod.description && <p className="mod-catalog-description">{mod.description}</p>}
                  </div>
                  <button
                    className="button secondary mod-catalog-install"
                    disabled={busy || catalogBusy || installing !== null}
                    onClick={() => void installCatalogMod(mod)}
                  ><Download size={14} /> {isInstalling ? "Installing…" : "Install"}</button>
                </article>;
              })}
            </div> : <p className="muted">No matching packages in {catalog.community}.</p>}
          </> : <p className="mod-catalog-hint">Search Thunderstore to find packages supported by this game.</p>}
        </div>

        {state.mods.length === 0 ? <div className="mod-empty">
          <PackageCheck size={19} />
          <span>{activeProfile?.name || "This profile"} has no mods yet. Browse Thunderstore or import an extracted mod folder.</span>
        </div> : <div className="mod-list">
          {state.mods.map((mod) => <article className="mod-row" key={mod.id}>
            <div className="mod-row-copy">
              <strong>{mod.name}</strong>
              <span>{mod.fileCount} files · {formatBytes(mod.totalBytes)} · {mod.enabled
                ? mod.deployedFileCount === mod.fileCount ? "Enabled" : `Needs review · ${mod.deployedFileCount}/${mod.fileCount} deployed`
                : "Disabled"}</span>
              {mod.dependencies.length > 0 && <span>Requires {mod.dependencies.join(", ")}</span>}
            </div>
            <div className="mod-row-actions">
              <button
                className={`mod-enabled-toggle${mod.enabled ? " is-enabled" : ""}`}
                aria-pressed={mod.enabled}
                disabled={busy}
                onClick={() => void toggleMod(mod.id, mod.name, !mod.enabled)}
              >{mod.enabled ? "Disable" : "Enable"}</button>
              <button className="icon-button" title="Open stored mod folder" aria-label={`Open ${mod.name} folder`}
                disabled={busy} onClick={() => void api.openModFolder(mod.id).catch((error) => props.onToast(String(error), "error"))}>
                <FolderOpen size={15} />
              </button>
              <button className="icon-button danger" title="Remove mod" aria-label={`Remove ${mod.name}`}
                disabled={busy} onClick={() => void removeMod(mod.id, mod.name, mod.enabled)}>
                <Trash2 size={14} />
              </button>
            </div>
          </article>)}
        </div>}
      </> : <p className="muted">Mod manager could not load.</p>}
      {newProfileOpen && (
        <NameEditDialog
          title="New mod profile"
          description={`Create a separate set of enabled mods for ${props.game.title}.`}
          initialName=""
          maxLength={40}
          submitLabel="Create profile"
          onSave={createProfile}
          onClose={() => setNewProfileOpen(false)}
        />
      )}
    </div>
  );
}
