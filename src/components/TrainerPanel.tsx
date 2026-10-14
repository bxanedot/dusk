import { Check, Gamepad2, LoaderCircle, Upload, Zap } from "lucide-react";
import { useEffect, useState } from "react";
import type { CheatTable } from "../types";
import { api } from "../lib/api";

type TrainerPanelProps = {
  gameId: string;
  gameTitle: string;
  hasExecutable: boolean;
  onToast: (message: string, type?: "ok" | "error") => void;
};

export function TrainerPanel(props: TrainerPanelProps) {
  const [table, setTable] = useState<CheatTable | null>(null);
  const [loading, setLoading] = useState(true);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [heldEntries, setHeldEntries] = useState<Set<string>>(() => new Set());
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    setLoading(true);
    setError(null);
    setTable(null);
    setHeldEntries(new Set());
    void api.getCheatTable(props.gameId)
      .then((result) => { if (current) setTable(result); })
      .catch((reason) => {
        if (current) setError(typeof reason === "string" ? reason : "Could not load this game's table.");
      })
      .finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [props.gameId]);

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
      setHeldEntries(new Set());
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

  async function applyValue(entryId: string, title: string) {
    setBusyAction(entryId);
    try {
      await api.applyCheatValue(props.gameId, entryId);
      props.onToast(`Applied “${title}”.`);
    } catch (reason) {
      props.onToast(typeof reason === "string" ? reason : "Could not apply this value.", "error");
    } finally {
      setBusyAction(null);
    }
  }

  async function toggleHold(entryId: string) {
    const enabling = !heldEntries.has(entryId);
    setBusyAction(entryId);
    try {
      await api.setCheatHold(props.gameId, entryId, enabling);
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
        For offline, single-player use. Dusk loads numeric values only; scripts, pointer paths, and custom types are skipped.
      </p>
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
            {table.entries.map((entry) => {
              const busy = busyAction === entry.id;
              const held = heldEntries.has(entry.id);
              return (
                <article className="trainer-entry" key={entry.id}>
                  <div className="trainer-entry-copy">
                    <strong title={entry.title}>{entry.title}</strong>
                    <span>{entry.variableType} · {entry.address} · value {entry.value}</span>
                  </div>
                  <div className="trainer-entry-actions">
                    <button
                      type="button"
                      className="button secondary trainer-entry-button"
                      onClick={() => void applyValue(entry.id, entry.title)}
                      disabled={busy || busyAction !== null || !props.hasExecutable}
                      title={!props.hasExecutable ? "Add this game's executable in Dusk first." : "Write this value once."}
                    >
                      {busy ? <LoaderCircle size={13} className="spin" /> : <Zap size={13} />}
                      Set
                    </button>
                    <button
                      type="button"
                      className={`button ${held ? "primary" : "secondary"} trainer-entry-button`}
                      onClick={() => void toggleHold(entry.id)}
                      disabled={busy || busyAction !== null || !props.hasExecutable}
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
        </>
      )}
    </section>
  );
}
