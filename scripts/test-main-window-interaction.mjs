// Windows CI interaction test: actually click the Dusk UI in headless Edge.
// Running a process and inspecting its exit code cannot detect a nonclickable
// WebView, an accidental click-blocking overlay, or a stuck account gate.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import { chromium } from "playwright-core";

const port = 1427;
const server = spawn(process.execPath,
  ["node_modules/vite/bin/vite.js", "--host", "127.0.0.1", "--port", String(port), "--strictPort"],
  { stdio: ["ignore", "pipe", "pipe"], windowsHide: true },
);
let serverError = "";
server.stderr.on("data", chunk => { serverError += chunk.toString(); });
let browser;
try {
  let available = false;
  for (let attempt = 0; attempt < 75; attempt++) {
    if (server.exitCode !== null) throw new Error("Vite failed: " + serverError);
    try {
      const response = await fetch("http://127.0.0.1:" + port);
      if (response.ok) { available = true; break; }
    } catch { /* server still starting */ }
    await sleep(200);
  }
  assert.ok(available, "Vite UI server did not start: " + serverError);

  browser = await chromium.launch({
    channel: "msedge",
    headless: true,
    timeout: 30000,
  });
  const context = await browser.newContext({ viewport: { width: 1500, height: 920 } });
  await context.addInitScript(() => {
    localStorage.setItem("dusk-account-mode", "guest");
    localStorage.setItem("dusk-auto-scan", "false");
    localStorage.setItem("dusk-windows-vpn-mode", "off");
    const stats = {
      gameCount: 0, favoriteCount: 0, playedGameCount: 0,
      totalSeconds: 0, launchCount: 0, last7DaysSeconds: 0,
      screenshotCount: 0, topGame: null,
    };
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: "main" },
        currentWebview: { label: "main", windowLabel: "main" },
      },
      transformCallback: () => 1,
      unregisterCallback: () => {},
      invoke: async (command) => {
        // Guest startup must not leave the account gate covering the app while
        // a native account-scope command is slow or unavailable.
        if (command === "set_account_scope") return await new Promise(() => {});
        if (command === "get_stats") return stats;
        if (command === "get_active_profile") return null;
        if (command === "refresh_missing_covers") return { updated: 0, attempted: 0, remaining: 0 };
        if (command === "list_windows_vpn_profiles") return [];
        if (command === "prepare_windows_vpn") return { connected: false, profile: null, message: "No VPN configured" };
        if (command === "data_directory") return "C:\\DuskTest";
        if (command === "list_managed_downloads") return [];
        if (command === "get_auto_scan") return false;
        if (/^(list_|get_|export_|scan_|collection_memberships)/.test(command)) return [];
        return null;
      },
    };
  });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("http://127.0.0.1:" + port, { waitUntil: "domcontentloaded" });
  const nav = page.locator(".nav-item");
  await nav.first().waitFor({ state: "visible", timeout: 20000 });

  async function clickAndCheck(label) {
    const button = nav.filter({ hasText: label }).first();
    await button.click({ timeout: 8000 });
    await page.waitForFunction(expected => {
      const active = document.querySelector(".nav-item.active");
      return !!active && (active.textContent || "").includes(expected);
    }, label, { timeout: 8000 });
    const text = (await page.locator(".nav-item.active").innerText()).trim();
    assert.ok(text.includes(label), "Click on " + label + " did not change the app view.");
  }

  await clickAndCheck("Library");
  await clickAndCheck("Favorites");
  await clickAndCheck("Screenshots");
  await clickAndCheck("Achievements");
  await clickAndCheck("Settings");
  await clickAndCheck("Home");
  // Explicitly check the physical click target, not just DOM handler dispatch.
  const home = nav.filter({ hasText: "Home" }).first();
  const blocked = await home.evaluate(element => {
    const rect = element.getBoundingClientRect();
    const target = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2);
    return target !== element && !element.contains(target);
  });
  assert.equal(blocked, false, "An overlay is physically covering the Home button.");
  assert.deepEqual(errors, [], "Frontend JavaScript exceptions: " + errors.join("; "));
  console.log("Dusk main window interaction smoke test passed: clickable Home, Library, Favorites, Screenshots, Achievements, Settings.");
} finally {
  await browser?.close();
  server.kill("SIGTERM");
}
