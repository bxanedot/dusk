import { Check, ChevronLeft, ChevronRight, ExternalLink, Gamepad2, LoaderCircle, Search, Upload, Zap } from "lucide-react";
import { type FormEvent, useEffect, useRef, useState } from "react";
import type { CheatTable, OnlineCheatTableResult } from "../types";
import { api } from "../lib/api";
import { onlineCheatTableKey, OnlineCheatTableResults, type OnlineCheatTableSearchState } from "./OnlineCheatTableResults";

type TrainerPanelProps = {
  gameId: string;
  gameTitle: string;
  hasExecutable: boolean;
  onToast: (message: string, type?: "ok" | "error") => void;
};

const ENTRY_PAGE_SIZE = 100;

export function TrainerPanel(props: TrainerPanelProps) {
  const [table, setTable] = useState<CheatTable | null>(null);
  const [loading, setLoading] = useState(true);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [heldEntries, setHeldEntries] = useState<Set<string>>(() => new Set());
  const [entryValues, setEntryValues] = useState<Record<string, string>>({});
  const [entryPage, setEntryPage] = useState(1);
  const [error, setError] = useState<string | null>(null);
  const [onlineState, setOnlineState] = useState<OnlineCheatTableSearchState>("idle");
  const [onlineResults, setOnlineResults] = useState<OnlineCheatTableResult[]>([]);
  const [onlineError, setOnlineError] = useState<string | null>(null);
  const onlineRequest = useRef(0);

  useEffect(() => {
    let current = true;
    setLoading(true);
    setError(null);
    setTable(null);
    setHeldEntries(new Set());
    setEntryValues({});
    setEntryPage(1);
    void api.getCheatTable(props.gameId)
      .then((result) => { if (current) setTable(result); })
      .catch((reason) => {
        if (current) setError(typeof reason === "string" ? reason : "Could not load this game's table.");
      })
      .finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [props.gameId]);

  const entryPageCount = table ? Math.max(1, Math.ceil(table.entries.length / ENTRY_PAGE_SIZE)) : 1;
  const currentEntryPage = Math.min(entryPage, entryPageCount);
  const visibleEntries = table?.entries.slice(
    (currentEntryPage - 1) * ENTRY_PAGE_SIZE,
    currentEntryPage * ENTRY_PAGE_SIZE,
  ) ?? [];

  useEffect(() => {
    onlineRequest.current += 1;
    setOnlineState("idle");
    setOnlineResults([]);
    setOnlineError(null);
    return () => { onlineRequest.current += 1; };
  }, [props.gameId, props.gameTitle]);

  async function searchOnlineTables(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const requestId = ++onlineRequest.current;
    setOnlineState("loading");
    setOnlineResults([]);
    setOnlineError(null);
    try {
      const results = await api.searchOnlineCheatTables(props.gameTitle);
      if (onlineRequest.current !== requestId) return;
      setOnlineResults(results);
      setOnlineState(results.length > 0 ? "results" : "empty");
    } catch (reason) {
      if (onlineRequest.current !== requestId) return;
      const message = typeof reason === "string" ? reason : "Could not search online tables.";
      setOnlineError(message);
      setOnlineState("error");
    }
  }

  async function searchNexusMods() {
    try {
      await api.openNexusModsSearch(props.gameTitle);
    } catch (reason) {
      const message = typeof reason === "string" ? reason : "Could not open Nexus Mods search.";
      props.onToast(message, "error");
    }
  }

  async function searchTableDirectory() {
    try {
      await api.openCheatTableDirectorySearch(props.gameTitle);
    } catch (reason) {
      const message = typeof reason === "string" ? reason : "Could not open table directory search.";
      props.onToast(message, "error");
    }
  }

  async function searchFearlessRevolution() {
    try {
      await api.openFearlessRevolutionSearch(props.gameTitle);
    } catch (reason) {
      const message = typeof reason === "string" ? reason : "Could not open Fearless Revolution search.";
      props.onToast(message, "error");
    }
  }

  async function importTable() {
    setBusyAction("import");
    setError(null);
    try {
      const imported = await api.importCheatTable(props.gameId);
      if (!imported) return;
      await Promise.all(Array.from(heldEntries, (entryId) =>
        api.setCheatHold(props.gameId, entryId, false).catch(() => undefined),
      ));
      setTable(imported);
      setEntryPage(1);
      setHeldEntries(new Set());
      setEntryValues({});
      const skipped = imported.skippedEntries > 0
        ? ` ${imported.skippedEntries} unsupported entr${imported.skippedEntries === 1 ? "y was" : "ies were"} skipped.`
        : "";
      props.onToast(`Imported ${imported.entries.length} entries for ${props.gameTitle}.${skipped}`);
    } catch (reason) {
      const message = typeof reason === "string" ? reason : "Could not import that table.";
      setError(message);
      props.onToast(message, "error");
    } finally {
      setBusyAction(null);
    }
  }

  async function importOnlineTable(result: OnlineCheatTableResult) {
    if (table && !window.confirm(`Replace the current table for ${props.gameTitle} with ${result.fileName}?`)) return;
    const actionId = onlineCheatTableKey(result);
    setBusyAction(actionId);
    setError(null);
    try {
      const imported = await api.importOnlineCheatTable(props.gameId, result);
      await Promise.all(Array.from(heldEntries, (entryId) =>
        api.setCheatHold(props.gameId, entryId, false).catch(() => undefined),
      ));
      setTable(imported);
      setEntryPage(1);
      setHeldEntries(new Set());
      setEntryValues({});
      setError(null);
      const skipped = imported.skippedEntries > 0
        ? ` ${imported.skippedEntries} unsupported entr${imported.skippedEntries === 1 ? "y was" : "ies were"} skipped.`
        : "";
      props.onToast(`Imported ${imported.entries.length} entries from ${result.fileName} for ${props.gameTitle}.${skipped}`);
    } catch (reason) {
      const message = typeof reason === "string" ? reason : "Could not download or import that table.";
      setError(message);
      props.onToast(message, "error");
    } finally {
      setBusyAction(null);
    }
  }

  async function applyValue(entryId: string, title: string, value: string) {
    setBusyAction(entryId);
    try {
      await api.applyCheatValue(props.gameId, entryId, value);
      props.onToast(`Applied “${title}”.`);
    } catch (reason) {
      props.onToast(typeof reason === "string" ? reason : "Could not apply this value.", "error");
    } finally {
      setBusyAction(null);
    }
  }

  async function toggleHold(entryId: string, value: string) {
    const enabling = !heldEntries.has(entryId);
    setBusyAction(entryId);
    try {
      await api.setCheatHold(props.gameId, entryId, enabling, value);
      setHeldEntries((current) => {
        const next = new Set(current);
        if (enabling) next.add(entryId);
        else next.delete(entryId);
        return next;
      });
    } catch (reason) {
      props.onToast(typeof reason === "string" ? reason : "Could not change this value.", "error");
    } finally {
      setBusyAction(null);
    }
  }

  return (
    <section className="detail-section trainer-panel" aria-label="Dusk Trainer">
      <div className="trainer-panel-header">
        <div className="trainer-catalog-heading-copy">
          <span className="trainer-catalog-mark" aria-hidden="true"><Gamepad2 size={17} /></span>
          <div>
            <h3>Dusk Trainer</h3>
            <p>Load a local Cheat Engine table and control supported values here.</p>
          </div>
        </div>
        <button
          type="button"
          className="button secondary trainer-catalog-search"
          onClick={() => void importTable()}
          disabled={busyAction !== null}
          aria-busy={busyAction === "import"}
        >
          {busyAction === "import" ? <LoaderCircle size={14} className="spin" /> : <Upload size={14} />}
          Import .CT
        </button>
      </div>

      <p className="trainer-panel-note">
        For offline, single-player use. Set a value for each numeric entry before applying or holding it. Scripts, pointer paths, and custom types are skipped.
      </p>
      <form className="trainer-online-search" onSubmit={(event) => void searchOnlineTables(event)}>
        <button type="submit" className="button secondary trainer-online-search-button" disabled={onlineState === "loading" || busyAction !== null}>
          {onlineState === "loading" ? <LoaderCircle size={14} className="spin" /> : <Search size={14} />}
          {onlineState === "loading" ? "Searching…" : "Search .CT online"}
        </button>
        <button type="button" className="button secondary trainer-online-search-button trainer-online-nexus-button" onClick={() => void searchNexusMods()} title="Open a free search on Nexus Mods in your browser">
          <ExternalLink size={14} /> Nexus Mods
        </button>
        <button type="button" className="button secondary trainer-online-search-button trainer-online-directory-button" onClick={() => void searchTableDirectory()} title="Search the free table directory listed by the community index">
          <ExternalLink size={14} /> CheatTable.net
        </button>
        <button type="button" className="button secondary trainer-online-search-button trainer-online-fearless-button" onClick={() => void searchFearlessRevolution()} title="Search Fearless Revolution in your browser">
          <ExternalLink size={14} /> Fearless Revolution
        </button>
      </form>
      <OnlineCheatTableResults
        state={onlineState}
        results={onlineResults}
        errorMessage={onlineError}
        busyId={busyAction}
        onImport={(result) => void importOnlineTable(result)}
      />
      {!props.hasExecutable && (
        <p className="trainer-panel-note">Add this game's executable in the Dusk library before applying or holding values.</p>
      )}

      {loading && (
        <div className="trainer-catalog-state" role="status">
          <LoaderCircle size={15} className="spin" />
          <span>Loading this game's table…</span>
        </div>
      )}
      {!loading && error && <div className="trainer-catalog-state trainer-catalog-error" role="alert">{error}</div>}
      {!loading && !table && !error && (
        <div className="trainer-catalog-state" role="status">
          <Upload size={15} />
          <span>Import a .CT table to add usable entries for this game. Tables stay on this PC.</span>
        </div>
      )}
      {!loading && table && (
        <>
          <div className="trainer-table-summary">
            <span title={table.sourceFileName}>{table.sourceFileName}</span>
            <span>{table.entries.length} entries</span>
          </div>
          <div className="trainer-entry-list">
            {visibleEntries.map((entry) => {
              const busy = busyAction === entry.id;
              const held = heldEntries.has(entry.id);
              const value = entryValues[entry.id] ?? entry.value;
              return (
                <article className="trainer-entry" key={entry.id}>
                  <div className="trainer-entry-copy">
                    <strong title={entry.title}>{entry.title}</strong>
                    <span>{entry.variableType} · {entry.address}</span>
                  </div>
                  <label className="trainer-entry-value">
                    <span>Value</span>
                    <input
                      type="text"
                      inputMode={entry.showAsHex ? "text" : "decimal"}
                      value={value}
                      maxLength={64}
                      placeholder={entry.showAsHex ? "Hex value" : "Enter value"}
                      aria-label={`Value for ${entry.title}`}
                      disabled={busyAction !== null}
                      onChange={(event) => setEntryValues((current) => ({ ...current, [entry.id]: event.target.value }))}
                    />
                  </label>
                  <div className="trainer-entry-actions">
                    <button
                      type="button"
                      className="button secondary trainer-entry-button"
                      onClick={() => void applyValue(entry.id, entry.title, value)}
                      disabled={busy || busyAction !== null || !props.hasExecutable || !value.trim()}
                      title={!props.hasExecutable ? "Add this game's executable in Dusk first." : "Write this value once."}
                    >
                      {busy ? <LoaderCircle size={13} className="spin" /> : <Zap size={13} />}
                      Set
                    </button>
                    <button
                      type="button"
                      className={`button ${held ? "primary" : "secondary"} trainer-entry-button`}
                      onClick={() => void toggleHold(entry.id, value)}
                      disabled={busy || busyAction !== null || !props.hasExecutable || !value.trim()}
                      aria-pressed={held}
                      title={!props.hasExecutable ? "Add this game's executable in Dusk first." : "Keep writing this value while the game is running."}
                    >
                      {busy ? <LoaderCircle size={13} className="spin" /> : held ? <Check size={13} /> : <Zap size={13} />}
                      {held ? "Holding" : "Hold"}
                    </button>
                  </div>
                </article>
              );
            })}
          </div>
          {entryPageCount > 1 && (
            <nav className="trainer-catalog-pagination trainer-entry-pagination" aria-label="Table entry pages">
              <button
                type="button"
                className="button secondary"
                onClick={() => setEntryPage(currentEntryPage - 1)}
                disabled={currentEntryPage <= 1 || busyAction !== null}
                aria-label="Previous table entry page"
              >
                <ChevronLeft size={15} /> Previous
              </button>
              <span>Entries {((currentEntryPage - 1) * ENTRY_PAGE_SIZE + 1).toLocaleString()}–{Math.min(currentEntryPage * ENTRY_PAGE_SIZE, table.entries.length).toLocaleString()} of {table.entries.length.toLocaleString()}</span>
              <button
                type="button"
                className="button secondary"
                onClick={() => setEntryPage(currentEntryPage + 1)}
                disabled={currentEntryPage >= entryPageCount || busyAction !== null}
                aria-label="Next table entry page"
              >
                Next page <ChevronRight size={15} />
              </button>
            </nav>
          )}
        </>
      )}
    </section>
  );
}
