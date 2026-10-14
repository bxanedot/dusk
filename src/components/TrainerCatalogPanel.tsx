import { Gamepad2, LoaderCircle, Search } from "lucide-react";
import type { TrainerCatalogResult } from "../types";

export type TrainerCatalogPanelState = "idle" | "loading" | "error" | "empty" | "results";

type TrainerCatalogPanelProps = {
  gameTitle: string;
  state: TrainerCatalogPanelState;
  results: TrainerCatalogResult[];
  openingKey: string | null;
  errorMessage: string | null;
  onSearch: () => void;
  onOpen: (result: TrainerCatalogResult) => void;
};

const MAX_CATALOG_ID = 18446744073709551615n;

function isValidCatalogId(value: string | null): boolean {
  if (!value || !/^[0-9]+$/.test(value)) return false;
  try {
    const numeric = BigInt(value);
    return numeric > 0n && numeric <= MAX_CATALOG_ID;
  } catch {
    return false;
  }
}

export function canOpenTrainer(result: TrainerCatalogResult): boolean {
  return result.status === "available"
    && isValidCatalogId(result.titleId)
    && isValidCatalogId(result.gameId);
}

function statusLabel(status: TrainerCatalogResult["status"]): string {
  switch (status) {
    case "available": return "Available";
    case "unsupported": return "Unsupported";
    case "unavailable": return "Unavailable";
    default: return "Status unknown";
  }
}

export function trainerResultKey(result: TrainerCatalogResult): string | null {
  if (!isValidCatalogId(result.titleId) || !isValidCatalogId(result.gameId)) return null;
  return result.titleId + ":" + result.gameId;
}

export function TrainerCatalogPanel(props: TrainerCatalogPanelProps) {
  const searching = props.state === "loading";

  return (
    <section className="detail-section trainer-catalog" aria-label="Trainer Catalog">
      <div className="trainer-catalog-header">
        <div className="trainer-catalog-heading-copy">
          <span className="trainer-catalog-mark" aria-hidden="true">
            <Gamepad2 size={17} />
          </span>
          <div>
            <h3>Trainer Catalog</h3>
            <p>Find trainers and support details for {props.gameTitle}.</p>
          </div>
        </div>
        <button
          type="button"
          className="button secondary trainer-catalog-search"
          onClick={props.onSearch}
          disabled={searching}
          aria-busy={searching}
        >
          {searching ? <LoaderCircle size={14} className="spin" /> : <Search size={14} />}
          {searching ? "Searching…" : "Search catalog"}
        </button>
      </div>

      {props.state === "idle" && (
        <div className="trainer-catalog-state" role="status">
          <Search size={15} />
          <span>Search the catalog to check trainer availability.</span>
        </div>
      )}
      {props.state === "loading" && (
        <div className="trainer-catalog-state" role="status">
          <LoaderCircle size={15} className="spin" />
          <span>Searching catalog…</span>
        </div>
      )}
      {props.state === "error" && (
        <div className="trainer-catalog-state trainer-catalog-error" role="alert">
          <span>{props.errorMessage || "Could not load trainer results. Try again."}</span>
        </div>
      )}
      {props.state === "empty" && (
        <div className="trainer-catalog-state trainer-catalog-empty" role="status">
          <span>No trainer matches were found for this game.</span>
        </div>
      )}
      {props.state === "results" && (
        <div className="trainer-result-list" aria-live="polite">
          {props.results.map((result, index) => {
            const launchable = canOpenTrainer(result);
            const key = trainerResultKey(result);
            const opening = key !== null && props.openingKey === key;
            const metadata = [
              result.platforms.length > 0 ? result.platforms.join(" · ") : null,
              result.cheatCount !== null ? `${result.cheatCount} cheats` : null,
              result.updated ? `Updated ${result.updated}` : null,
            ].filter((part): part is string => Boolean(part));

            return (
              <article className="trainer-result" key={key || result.title + ":" + index}>
                <div className="trainer-result-main">
                  <div className="trainer-result-heading">
                    <strong title={result.title}>{result.title}</strong>
                    <span className={`trainer-status trainer-status-${result.status}`}>
                      {statusLabel(result.status)}
                    </span>
                  </div>
                  <div className="trainer-result-meta">
                    {metadata.length > 0 ? metadata.join(" · ") : "Details not available"}
                  </div>
                </div>
                {launchable && (
                  <button
                    type="button"
                    className="button primary trainer-open"
                    onClick={() => props.onOpen(result)}
                    disabled={opening}
                    aria-label={`Open trainer for ${result.title}`}
                  >
                    {opening && <LoaderCircle size={13} className="spin" />}
                    {opening ? "Opening…" : "Open trainer"}
                  </button>
                )}
              </article>
            );
          })}
        </div>
      )}

      <p className="trainer-catalog-note">Trainer controls are handled by the trainer app.</p>
    </section>
  );
}
