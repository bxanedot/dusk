import { useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { ChevronLeft, ChevronRight, Library } from "lucide-react";
import type { GameRecord } from "../types";

interface GameRailProps {
  games: GameRecord[];
  selectedGameId: string | null;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onSelectGame: (gameId: string) => void;
  onOpenLibrary: () => void;
}

function RailCover({ game }: { game: GameRecord }) {
  const [failed, setFailed] = useState(false);
  const src = game.coverDataUrl || (game.coverPath ? convertFileSrc(game.coverPath) : null);
  return <span className="rail-cover">
    {src && !failed ? <img src={src} alt="" loading="lazy" onError={() => setFailed(true)} />
      : <span>{game.title.split(/\s+/).slice(0, 2).map(word => word[0]).join("").toUpperCase() || "D"}</span>}
  </span>;
}

export function GameRail(props: GameRailProps) {
  return <aside className={"game-rail" + (props.collapsed ? " is-collapsed" : "")} aria-label="Your games">
    <div className="game-rail-heading">
      <span>Your games <small>{props.games.length}</small></span>
      <button className="icon-button" onClick={props.onToggleCollapsed} aria-expanded={!props.collapsed}
        aria-label={props.collapsed ? "Expand game rail" : "Collapse game rail"} title={props.collapsed ? "Expand game rail" : "Collapse game rail"}>
        {props.collapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
      </button>
    </div>
    <nav className="game-rail-list" aria-label="Local game library">
      {props.games.map(game => <button key={game.id} className={"game-rail-item" + (props.selectedGameId === game.id ? " active" : "")}
        data-controller-game-id={game.id} title={game.title} aria-label={game.title}
        aria-current={props.selectedGameId === game.id ? "page" : undefined} onClick={() => props.onSelectGame(game.id)}>
        <RailCover key={game.coverDataUrl || game.coverPath || game.id} game={game} />
        <span className="game-rail-copy"><strong>{game.title}</strong><small>{game.source} {game.favorite ? "· Favorite" : ""}</small></span>
      </button>)}
      {props.games.length === 0 && <p className="rail-empty">Your games will appear here. Scan your PC or add a game to get started.</p>}
    </nav>
    <button className="game-rail-library" onClick={props.onOpenLibrary} title="Open library" aria-label="Open library">
      <Library size={18} /><span>All games</span>
    </button>
  </aside>;
}
