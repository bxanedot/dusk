import { Cloud, Library, Plus, ScanSearch, X } from "lucide-react";
import duskLogo from "../assets/dusk-logo.svg";

export function FirstRunOnboarding(props: {
  scanning: boolean;
  onScan: () => void;
  onAdd: () => void;
  onClose: () => void;
}) {
  return (
    <div className="modal-backdrop onboarding-backdrop" onMouseDown={props.onClose}>
      <section
        className="onboarding-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="onboarding-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <button className="icon-button onboarding-close" aria-label="Close welcome" onClick={props.onClose}>
          <X size={17} />
        </button>
        <div className="onboarding-brand">
          <img src={duskLogo} alt="" draggable={false} />
          <span>DUSK</span>
        </div>
        <span className="eyebrow">Your library starts here</span>
        <h1 id="onboarding-title">Welcome to Dusk.</h1>
        <p className="onboarding-copy">Bring your installed games together, then manage their saves, mods, and playtime from one place.</p>
        <div className="onboarding-actions">
          <button className="button primary" onClick={props.onScan} disabled={props.scanning}>
            <ScanSearch size={16} />
            {props.scanning ? "Scanning this PC…" : "Scan this PC"}
          </button>
          <button className="button secondary" onClick={props.onAdd}>
            <Plus size={16} />
            Add a game
          </button>
        </div>
        <div className="onboarding-note">
          <Library size={16} />
          <span>Games and save files stay on this PC.</span>
        </div>
        <div className="onboarding-note">
          <Cloud size={16} />
          <span>When signed in and online, Dusk syncs your playtime and achievements to Supabase.</span>
        </div>
        <button className="onboarding-later" onClick={props.onClose}>I’ll set it up later</button>
      </section>
    </div>
  );
}
