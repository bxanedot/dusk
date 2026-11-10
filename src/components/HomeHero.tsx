import { useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Play } from "../icons";
import type { GameRecord } from "../types";

interface HomeHeroProps {
  game: GameRecord;
  meta: string;
  playing: boolean;
  onPlay: () => void;
  onOpen: () => void;
}

/** The game to jump back into, shown large at the top of Home like a launcher's featured banner. */
export function HomeHero({ game, meta, playing, onPlay, onOpen }: HomeHeroProps) {
  const [failed, setFailed] = useState(false);
  const src = failed ? null : game.coverDataUrl || (game.coverPath ? convertFileSrc(game.coverPath) : null);
  return (
    <section className="home-hero" aria-label={`Continue ${game.title}`}>
      {src && <img className="home-hero-backdrop" src={src} alt="" aria-hidden="true" onError={() => setFailed(true)} />}
      <div className="home-hero-copy">
        <span className="home-hero-label">Jump back in</span>
        <h1>{game.title}</h1>
        <p>{meta}</p>
        <div className="home-hero-actions">
          <button className="button primary hero-play" onClick={onPlay} disabled={playing}>
            <Play size={18} fill="currentColor" />
            {playing ? "Playing" : "Play"}
          </button>
          <button className="button secondary" onClick={onOpen}>Details</button>
        </div>
      </div>
      {src && <img className="home-hero-poster" src={src} alt="" />}
    </section>
  );
}
