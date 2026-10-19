import { Gamepad2, LoaderCircle, Search } from "lucide-react";
import type { TrainerCatalogResult } from "../types";

export type TrainerCatalogPanelState = "idle" | "loading" | "error" | "empty" | "results";

type TrainerCatalogPanelProps = {
  gameTitle: string;
  state: TrainerCatalogPanelState;
  results: TrainerCatalogResult[];
  errorMessage: string | null;
  onSearch: () => void;
  onBrowse: () => void;
};

function statusLabel(status: TrainerCatalogResult["status"]): string {
  switch (status) {
    case "available": return "Available";
    case "unsupported": return "Unsupported";
    case "unavailable": return "Unavailable";
    default: return "Status unknown";
  }
}

export function TrainerCatalogPanel(props: TrainerCatalogPanelProps) {
  const searching = props.state === "loading";

  return (
    <div className="trainer-catalog">
      <div className="trainer-catalog-header">
        <div className="trainer-catalog-heading-copy">
          <span className="trainer-catalog-mark" aria-hidden="true">
            <Gamepad2 size={17} />
          </span>
          <div>
            <h3>Dusk Catalog</h3>
            <p>Check title and platform coverage for {props.gameTitle}. Import a local table below to use values.</p>
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
          <span>Search the offline catalog for this game's title and platform coverage.</span>
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
          <div>
            <span>No title or known alias matches this game in the current catalog snapshot.</span>
          </div>
          <button type="button" className="button secondary" onClick={props.onBrowse}>
            Browse full catalog
          </button>
        </div>
      )}
      {props.state === "results" && (
        <div className="trainer-result-list" aria-live="polite">
          {props.results.map((result, index) => {
            const metadata = [
              result.platforms.length > 0 ? result.platforms.join(" · ") : null,
              result.cheatCount !== null ? `Up to ${result.cheatCount} cheats` : null,
            ].filter((part): part is string => Boolean(part));

            return (
              <article className="trainer-result" key={result.title + ":" + index}>
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
              </article>
            );
          })}
        </div>
      )}

    </div>
  );
}
