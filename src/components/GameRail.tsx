import { useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { GameRecord } from "../types";

interface GameRailProps {
  games: GameRecord[];
  selectedGameId: string | null;
  onSelectGame: (gameId: string) => void;
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
  return <section className="game-rail" aria-label="Your games">
    <div className="game-rail-heading">Your Games <small>{props.games.length}</small></div>
    <nav className="game-rail-list" aria-label="Local game library">
      {props.games.map(game => <button key={game.id} className={"game-rail-item" + (props.selectedGameId === game.id ? " active" : "")}
        data-controller-game-id={game.id} title={game.title} aria-label={game.title}
        aria-current={props.selectedGameId === game.id ? "page" : undefined} onClick={() => props.onSelectGame(game.id)}>
        <RailCover key={game.coverDataUrl || game.coverPath || game.id} game={game} />
        <span className="game-rail-copy"><strong>{game.title}</strong></span>
      </button>)}
      {props.games.length === 0 && <p className="rail-empty">Scan your PC or add a game and it shows up here.</p>}
    </nav>
  </section>;
}
