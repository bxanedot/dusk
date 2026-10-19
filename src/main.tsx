import { StrictMode } from "react";
import { Image } from "@tauri-apps/api/image";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createRoot } from "react-dom/client";
import App from "./App";
import AccountGate from "./AccountGate";
import duskWindowIcon from "./assets/dusk-logo.png";
import "./styles.css";

async function applyDuskWindowIcon() {
  try {
    const response = await fetch(duskWindowIcon);
    if (!response.ok) return;

    const icon = await Image.fromBytes(await response.arrayBuffer());
    try {
      await getCurrentWindow().setIcon(icon);
    } finally {
      await icon.close();
    }
  } catch {
    // The web dev server can run outside Tauri; the packaged app sets the icon.
  }
}

void applyDuskWindowIcon();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <AccountGate><App /></AccountGate>
  </StrictMode>,
);
