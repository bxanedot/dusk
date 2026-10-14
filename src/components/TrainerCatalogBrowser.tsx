import { ChevronLeft, ChevronRight, ExternalLink, Gamepad2, LoaderCircle, Search } from "lucide-react";
import { type FormEvent, useState } from "react";
import type { OnlineCheatTableResult, TrainerCatalogResult } from "../types";
import { OnlineCheatTableResults, type OnlineCheatTableSearchState } from "./OnlineCheatTableResults";

export type TrainerCatalogBrowserState = "idle" | "loading" | "error" | "ready";
export type TrainerCatalogStatusFilter = "all" | TrainerCatalogResult["status"];

type TrainerCatalogBrowserProps = {
  state: TrainerCatalogBrowserState;
  results: TrainerCatalogResult[];
  query: string;
  statusFilter: TrainerCatalogStatusFilter;
  page: number;
  pageSize?: number;
  errorMessage: string | null;
  onlineTableState: OnlineCheatTableSearchState;
  onlineTableResults: OnlineCheatTableResult[];
  onlineTableError: string | null;
  onlineTableBusyId: string | null;
  onSelectGame: (result: TrainerCatalogResult) => void;
  onSearchOnlineTables: (query: string) => void;
  onSearchNexusMods: (query: string) => void;
  onImportOnlineTable: (result: OnlineCheatTableResult) => void;
  onQueryChange: (query: string) => void;
  onStatusFilterChange: (filter: TrainerCatalogStatusFilter) => void;
  onRetry: () => void;
  onPageChange: (page: number) => void;
};

export function normalizeCatalogText(value: string): string {
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
  const normalizedQuery = normalizeCatalogText(query);
  const tokens = normalizedQuery.split(/\s+/).filter(Boolean);
  const matches = results.flatMap((result) => {
    if (statusFilter !== "all" && result.status !== statusFilter) return [];
    if (tokens.length === 0) return [{ rank: 0, result }];
    const candidates = [result.title, ...(result.aliases ?? [])];
    let rank: number | null = null;
    for (const candidate of candidates) {
      const normalized = normalizeCatalogText(candidate);
      const words = normalized.split(/\s+/).filter(Boolean);
      const candidateRank = normalized === normalizedQuery
        ? 0
        : normalized.startsWith(normalizedQuery)
          ? 1
          : normalized.includes(normalizedQuery)
            ? 2
            : tokens.every((token) => words.some((word) => token.length === 1 ? word === token : word.startsWith(token)))
              ? 3
              : (() => {
                  const matchesWord = (token: string, word: string) => token.length === 1 ? word === token : word.startsWith(token);
                  const matchedQueryWords = tokens.filter((token) => words.some((word) => matchesWord(token, word))).length;
                  const matchedTitleWords = words.filter((word) => tokens.some((token) => matchesWord(token, word))).length;
                  return matchedQueryWords >= 2
                    && matchedQueryWords * 5 >= tokens.length * 3
                    && matchedTitleWords * 5 >= words.length * 3
                    ? 4
                    : null;
                })();
      if (candidateRank !== null && (rank === null || candidateRank < rank)) rank = candidateRank;
    }
    return rank === null ? [] : [{ rank, result }];
  });
  return matches
    .sort((left, right) => left.rank - right.rank || left.result.title.localeCompare(right.result.title))
    .map(({ result }) => result);
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
  const [onlineQuery, setOnlineQuery] = useState("");
  const filteredResults = filterTrainerCatalogResults(props.results, props.query, props.statusFilter);
  const pageSize = Math.max(1, props.pageSize ?? 36);
  const pageCount = Math.max(1, Math.ceil(filteredResults.length / pageSize));
  const page = Math.min(Math.max(1, props.page), pageCount);
  const visibleResults = filteredResults.slice((page - 1) * pageSize, page * pageSize);
  const searching = props.state === "loading";

  function searchOnlineTables(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    props.onSearchOnlineTables(onlineQuery);
  }

  return (
    <div className="page trainer-catalog-page">
      <div className="page-heading trainer-catalog-page-heading">
        <div>
          <span className="eyebrow">Browse</span>
          <h1>Dusk Catalog</h1>
          <p>Select a title to open its game details. Add or scan the game in Dusk's library first to reach its local trainer controls.</p>
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

      <section className="trainer-online-catalog" aria-labelledby="trainer-online-catalog-title">
        <div className="trainer-online-catalog-heading">
          <div>
            <h2 id="trainer-online-catalog-title">Online .CT tables</h2>
            <p>Search public GitHub repositories. Dusk imports supported numeric entries and skips scripts and pointer paths. Community tables may be outdated, so check the source and game version.</p>
          </div>
        </div>
        <form className="trainer-online-search trainer-online-search-catalog" onSubmit={searchOnlineTables}>
          <label className="trainer-online-query">
            <Search size={16} aria-hidden="true" />
            <input
              type="search"
              value={onlineQuery}
              onChange={(event) => setOnlineQuery(event.target.value)}
              placeholder="Search by game title"
              aria-label="Search public .CT tables by game title"
              autoComplete="off"
            />
          </label>
          <button type="submit" className="button secondary trainer-online-search-button" disabled={props.onlineTableState === "loading" || props.onlineTableBusyId !== null}>
            {props.onlineTableState === "loading" ? <LoaderCircle size={14} className="spin" /> : <Search size={14} />}
            {props.onlineTableState === "loading" ? "Searching…" : "Search .CT online"}
          </button>
          <button type="button" className="button secondary trainer-online-search-button trainer-online-nexus-button" onClick={() => props.onSearchNexusMods(onlineQuery)} disabled={!onlineQuery.trim()} title="Open a free search on Nexus Mods in your browser">
            <ExternalLink size={14} /> Nexus Mods
          </button>
        </form>
        <OnlineCheatTableResults
          state={props.onlineTableState}
          results={props.onlineTableResults}
          errorMessage={props.onlineTableError}
          busyId={props.onlineTableBusyId}
          onImport={props.onImportOnlineTable}
        />
      </section>

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
              const metadata = [
                result.platforms.length > 0 ? result.platforms.join(" · ") : null,
                result.cheatCount !== null ? `Up to ${result.cheatCount} cheats` : null,
              ].filter((part): part is string => Boolean(part));

              return (
                <button
                  type="button"
                  className="trainer-catalog-card"
                  key={`${result.title}:${index}`}
                  onClick={() => props.onSelectGame(result)}
                  aria-label={`Open ${result.title} in your Dusk library`}
                  title={`Open ${result.title} in your Dusk library`}
                >
                  <span className="trainer-catalog-card-icon" aria-hidden="true"><Gamepad2 size={19} /></span>
                  <div className="trainer-catalog-card-body">
                    <div className="trainer-catalog-card-title">
                      <strong title={result.title}>{result.title}</strong>
                      <span className={`trainer-status trainer-status-${result.status}`}>{statusLabel(result.status)}</span>
                    </div>
                    <div className="trainer-catalog-card-meta">
                      {metadata.length > 0 ? metadata.join(" · ") : "Details not available"}
                    </div>
                  </div>
                </button>
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
        </>
      )}
    </div>
  );
}
