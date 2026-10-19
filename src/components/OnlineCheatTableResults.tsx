import { Download, FileCode2, LoaderCircle, Star } from "lucide-react";
import type { OnlineCheatTableResult } from "../types";

export type OnlineCheatTableSearchState = "idle" | "loading" | "empty" | "error" | "results";

type OnlineCheatTableResultsProps = {
  state: OnlineCheatTableSearchState;
  results: OnlineCheatTableResult[];
  errorMessage: string | null;
  busyId: string | null;
  onImport: (result: OnlineCheatTableResult) => void;
};

export function onlineCheatTableKey(result: OnlineCheatTableResult): string {
  return `${result.repository}:${result.path}:${result.blobSha}`;
}

function formatSize(sizeBytes: number): string {
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  if (sizeBytes < 1024 * 1024) return `${Math.ceil(sizeBytes / 1024)} KB`;
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function OnlineCheatTableResults(props: OnlineCheatTableResultsProps) {
  if (props.state === "idle") {
    return (
      <div className="trainer-catalog-state" role="status">
        <FileCode2 size={15} />
        <span>Search public GitHub repositories for .CT files.</span>
      </div>
    );
  }
  if (props.state === "loading") {
    return (
      <div className="trainer-catalog-state" role="status" aria-busy="true">
        <LoaderCircle size={15} className="spin" />
        <span>Searching public .CT files…</span>
      </div>
    );
  }
  if (props.state === "error") {
    return <div className="trainer-catalog-state trainer-catalog-error" role="alert">{props.errorMessage || "Could not search online tables."}</div>;
  }
  if (props.state === "empty") {
    return (
      <div className="trainer-catalog-state trainer-catalog-empty" role="status">
        <FileCode2 size={15} />
        <span>No public .CT files matched. Try a shorter game title.</span>
      </div>
    );
  }

  return (
    <div className="trainer-online-results" aria-live="polite">
      {props.results.map((result) => {
        const key = onlineCheatTableKey(result);
        const busy = props.busyId === key;
        return (
          <article className="trainer-online-result" key={key}>
            <div className="trainer-online-result-copy">
              <strong title={result.fileName}>{result.fileName}</strong>
              <span title={result.path}>{result.repository} · {result.path}</span>
              <small><Star size={11} /> {result.stars.toLocaleString()} · {formatSize(result.sizeBytes)} · GitHub</small>
            </div>
            <button
              type="button"
              className="button secondary trainer-online-import"
              onClick={() => props.onImport(result)}
              disabled={props.busyId !== null}
              aria-busy={busy}
              title="Download and import supported numeric entries"
            >
              {busy ? <LoaderCircle size={13} className="spin" /> : <Download size={13} />}
              Import
            </button>
          </article>
        );
      })}
    </div>
  );
}
