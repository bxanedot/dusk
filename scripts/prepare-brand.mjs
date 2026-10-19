import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

if (process.platform === "win32") {
  const result = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", resolve(root, "scripts/prepare-brand.ps1")],
    { cwd: root, stdio: "inherit" },
  );
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
} else {
  const required = [
    "src/assets/dusk-logo.png",
    "src-tauri/icons/32x32.png",
    "src-tauri/icons/128x128.png",
    "src-tauri/icons/128x128@2x.png",
    "src-tauri/icons/icon.png",
    "src-tauri/icons/icon.ico",
  ];
  const missing = required.filter((path) => !existsSync(resolve(root, path)));
  if (missing.length) {
    throw new Error(`Missing checked-in Dusk branding assets: ${missing.join(", ")}`);
  }
}
