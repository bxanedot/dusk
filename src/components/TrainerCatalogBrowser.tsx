import { ChevronLeft, ChevronRight, Gamepad2, LoaderCircle, Search } from "lucide-react";
import { canOpenTrainer, trainerResultKey } from "./TrainerCatalogPanel";
import type { TrainerCatalogResult } from "../types";

export type TrainerCatalogBrowserState = "idle" | "loading" | "error" | "ready";
export type TrainerCatalogStatusFilter = "all" | TrainerCatalogResult["status"];

type TrainerCatalogBrowserProps = {
  state: TrainerCatalogBrowserState;
  results: TrainerCatalogResult[];
  query: string;
  statusFilter: TrainerCatalogStatusFilter;
  page: number;
  pageSize?: number;
  openingKey: string | null;
  errorMessage: string | null;
  onQueryChange: (query: string) => void;
  onStatusFilterChange: (filter: TrainerCatalogStatusFilter) => void;
  onRetry: () => void;
  onOpen: (result: TrainerCatalogResult) => void;
  onPageChange: (page: number) => void;
};

function normalizeCatalogText(value: string): string {
  return value
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .toLocaleLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();
}

export function filterTrainerCatalogResults(
  results: TrainerCatalogResult[],
  query: string,
  statusFilter: TrainerCatalogStatusFilter,
): TrainerCatalogResult[] {
  const tokens = normalizeCatalogText(query).split(/\s+/).filter(Boolean);
  return results.filter((result) => {
    if (statusFilter !== "all" && result.status !== statusFilter) return false;
    const searchable = normalizeCatalogText([result.title, ...result.platforms].join(" "));
    return tokens.every((token) => searchable.includes(token));
  });
}

function statusLabel(status: TrainerCatalogResult["status"]): string {
  switch (status) {
    case "available": return "Available";
    case "unsupported": return "Unsupported";
    case "unavailable": return "Unavailable";
    default: return "Status unknown";
  }
}

export function TrainerCatalogBrowser(props: TrainerCatalogBrowserProps) {
  const filteredResults = filterTrainerCatalogResults(props.results, props.query, props.statusFilter);
  const pageSize = Math.max(1, props.pageSize ?? 36);
  const pageCount = Math.max(1, Math.ceil(filteredResults.length / pageSize));
  const page = Math.min(Math.max(1, props.page), pageCount);
  const visibleResults = filteredResults.slice((page - 1) * pageSize, page * pageSize);
  const searching = props.state === "loading";

  return (
    <div className="page trainer-catalog-page">
      <div className="page-heading trainer-catalog-page-heading">
        <div>
          <span className="eyebrow">Browse</span>
          <h1>Trainer Catalog</h1>
          <p>Search games, check support details, and open available trainers.</p>
        </div>
        {props.state === "ready" && (
          <div className="trainer-catalog-total" aria-label={`${props.results.length} catalog entries`}>
            <strong>{props.results.length.toLocaleString()}</strong>
            <span>catalog entries</span>
          </div>
        )}
      </div>

      <div className="trainer-catalog-toolbar">
        <label className="trainer-catalog-query">
          <Search size={17} aria-hidden="true" />
          <input
            type="search"
            value={props.query}
            onChange={(event) => props.onQueryChange(event.target.value)}
            placeholder="Search all game titles"
            aria-label="Search catalog"
            autoComplete="off"
          />
        </label>
        <label className="trainer-catalog-status-filter">
          <span>Availability</span>
          <select
            value={props.statusFilter}
            onChange={(event) => props.onStatusFilterChange(event.target.value as TrainerCatalogStatusFilter)}
            aria-label="Filter by availability"
          >
            <option value="all">All entries</option>
            <option value="available">Available</option>
            <option value="unsupported">Unsupported</option>
            <option value="unavailable">Unavailable</option>
            <option value="unknown">Unknown</option>
          </select>
        </label>
        {props.state === "ready" && (
          <span className="trainer-catalog-result-count" role="status" aria-live="polite">
            {filteredResults.length.toLocaleString()} {filteredResults.length === 1 ? "result" : "results"}
          </span>
        )}
      </div>

      {props.state === "idle" && (
        <div className="trainer-catalog-page-state" role="status">
          <Gamepad2 size={19} />
          <span>Load the catalog to browse available entries.</span>
          <button type="button" className="button secondary" onClick={props.onRetry}>Load catalog</button>
        </div>
      )}
      {props.state === "loading" && (
        <div className="trainer-catalog-page-state" role="status" aria-busy="true">
          <LoaderCircle size={19} className="spin" />
          <span>Loading catalog entries…</span>
        </div>
      )}
      {props.state === "error" && (
        <div className="trainer-catalog-page-state trainer-catalog-page-error" role="alert">
          <span>{props.errorMessage || "Could not load the catalog. Try again."}</span>
          <button type="button" className="button secondary" onClick={props.onRetry}>Try again</button>
        </div>
      )}
      {props.state === "ready" && filteredResults.length === 0 && (
        <div className="trainer-catalog-page-state" role="status">
          <Search size={19} />
          <div>
            <strong>No matching entries</strong>
            <span>Try a shorter title or change the availability filter.</span>
          </div>
        </div>
      )}
      {props.state === "ready" && visibleResults.length > 0 && (
        <>
          <div className="trainer-catalog-grid" aria-label="Trainer catalog results" aria-live="polite">
            {visibleResults.map((result, index) => {
              const launchable = canOpenTrainer(result);
              const key = trainerResultKey(result);
              const opening = key !== null && props.openingKey === key;
              const metadata = [
                result.platforms.length > 0 ? result.platforms.join(" · ") : null,
                result.cheatCount !== null ? `${result.cheatCount} cheats` : null,
                result.updated ? `Updated ${result.updated}` : null,
              ].filter((part): part is string => Boolean(part));

              return (
                <article className="trainer-catalog-card" key={key || `${result.title}:${index}`}>
                  <span className="trainer-catalog-card-icon" aria-hidden="true"><Gamepad2 size={19} /></span>
                  <div className="trainer-catalog-card-body">
                    <div className="trainer-catalog-card-title">
                      <strong title={result.title}>{result.title}</strong>
                      <span className={`trainer-status trainer-status-${result.status}`}>{statusLabel(result.status)}</span>
                    </div>
                    <div className="trainer-catalog-card-meta">
                      {metadata.length > 0 ? metadata.join(" · ") : "Details not available"}
                    </div>
                    {launchable && (
                      <button
                        type="button"
                        className="button primary trainer-catalog-card-open"
                        onClick={() => props.onOpen(result)}
                        disabled={opening}
                        aria-label={`Open trainer for ${result.title}`}
                      >
                        {opening ? "Opening…" : "Open trainer"}
                      </button>
                    )}
                  </div>
                </article>
              );
            })}
          </div>
          {pageCount > 1 && (
            <nav className="trainer-catalog-pagination" aria-label="Catalog pages">
              <button
                type="button"
                className="button secondary"
                onClick={() => props.onPageChange(page - 1)}
                disabled={page <= 1 || searching}
                aria-label="Previous catalog page"
              >
                <ChevronLeft size={15} /> Previous
              </button>
              <span>Page {page} of {pageCount}</span>
              <button
                type="button"
                className="button secondary"
                onClick={() => props.onPageChange(page + 1)}
                disabled={page >= pageCount || searching}
                aria-label="Next catalog page"
              >
                Next page <ChevronRight size={15} />
              </button>
            </nav>
          )}
          <p className="trainer-catalog-page-note">Trainer controls are handled by the trainer app.</p>
        </>
      )}
    </div>
  );
}
