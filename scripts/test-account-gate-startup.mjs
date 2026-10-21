import assert from "node:assert/strict";
import { createServer } from "vite";
import { chromium } from "playwright-core";

const entryId = "\0dusk-account-gate-fixture";
const virtualModules = {
  "./lib/auth": `
    const account = { user: { id: "fixture-account" }, username: "fixture", displayName: "Fixture" };
    export const currentDuskAccount = async () => null;
    export const loginDuskAccount = async () => account;
    export const registerDuskAccount = async () => ({ account, message: "Registered" });
    export const requestDuskPasswordReset = async () => "Reset requested";
    export const supabaseConfigured = () => true;
  `,
  "./lib/accountSync": `
    export async function hydrateAccountState() {
      window.__hydrationCalls++;
      if (location.search.includes("failed")) throw new Error("Fixture cloud unavailable");
      return new Promise(() => {});
    }
  `,
  "./lib/api": `
    export const api = { setAccountScope: async id => {
      window.__accountScopes.push(id);
      if (id && location.search.includes("hold-scope")) {
        await new Promise(resolve => { window.__releaseScope = resolve; });
      }
    } };
  `,
  "@tauri-apps/api/window": `
    export const getCurrentWindow = () => ({
      minimize: async () => {}, toggleMaximize: async () => {}, close: async () => {}, startDragging: async () => {},
    });
  `,
};

const server = await createServer({
  clearScreen: false,
  logLevel: "error",
  server: { host: "127.0.0.1", port: 1431, strictPort: true },
  plugins: [{
    name: "account-gate-regression-fixture",
    enforce: "pre",
    resolveId(source, importer) {
      if (source === "virtual:dusk-account-gate-fixture") return entryId;
      if (importer?.replaceAll("\\", "/").endsWith("/src/AccountGate.tsx") && source in virtualModules) {
        return "\0account-gate:" + source;
      }
    },
    load(id) {
      if (id.startsWith("\0account-gate:")) return virtualModules[id.slice("\0account-gate:".length)];
      if (id === entryId) return `
        import React from "react";
        import { createRoot } from "react-dom/client";
        import AccountGate from "/src/AccountGate.tsx";
        import "/src/styles.css";
        window.__accountScopes = [];
        window.__hydrationCalls = 0;
        createRoot(document.getElementById("root")).render(
          React.createElement(AccountGate, null, React.createElement("div", { id: "library-ready" }, "Local library"))
        );
      `;
    },
    configureServer(vite) {
      vite.middlewares.use("/__account-gate", async (_request, response, next) => {
        try {
          const html = await vite.transformIndexHtml("/__account-gate.html", `
            <!doctype html><html><head></head><body><div id="root"></div>
            <script type="module">import "virtual:dusk-account-gate-fixture";</script></body></html>
          `);
          response.setHeader("Content-Type", "text/html");
          response.end(html);
        } catch (error) { next(error); }
      });
    },
  }],
});

let browser;
try {
  await server.listen();
  browser = await chromium.launch({ channel: "msedge", headless: true });
  for (const scenario of ["login", "register", "failed", "hold-scope"]) {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", error => errors.push(error.message));
    try {
      await page.goto("http://127.0.0.1:1431/__account-gate?" + scenario);
      await page.locator(".account-form").waitFor();
      if (scenario === "register") {
        await page.locator(".account-switch").click();
        await page.getByLabel("Display name", { exact: true }).fill("Fixture");
        await page.getByLabel("Email", { exact: true }).fill("fixture@example.com");
      }
      await page.getByLabel("Username", { exact: true }).fill("fixture");
      await page.getByLabel("Password", { exact: true }).fill("fixture-password");
      await page.locator(".account-submit").click();
      if (scenario === "hold-scope") {
        await page.waitForFunction(() => typeof window.__releaseScope === "function");
        assert.equal(await page.locator("#library-ready").count(), 0, "the native account scope must be ready first");
        await page.evaluate(() => window.__releaseScope());
      }
      await page.locator("#library-ready").waitFor({ timeout: 1500 });
      assert.equal(await page.evaluate(() => window.__accountScopes.at(-1)), "fixture-account");
      assert.equal(await page.evaluate(() => window.__hydrationCalls), 1);
      assert.deepEqual(errors, []);
    } finally { await page.close(); }
  }
  console.log("Account-gate startup checks passed: sign-in, registration, failed/stalled cloud restoration, and native scope ordering.");
} finally {
  await browser?.close();
  await server.close();
}
