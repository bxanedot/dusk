const assert = require("node:assert/strict");
const fs = require("node:fs");
const Module = require("node:module");
const path = require("node:path");
const { test } = require("node:test");
const ts = require("typescript");

function fixture(mode) {
  const settings = new Map(mode ? [["dusk-account-mode", mode]] : []);
  global.localStorage = {
    getItem: key => settings.get(key) ?? null,
    setItem: (key, value) => settings.set(key, value),
    removeItem: key => settings.delete(key),
  };
  const filename = path.resolve(__dirname, "../src/lib/accountMode.ts");
  const loaded = new Module(filename, module);
  loaded.filename = filename;
  loaded.require = request => {
    if (request !== "./accountStateRestore") return module.require(request);
    const helperPath = path.resolve(path.dirname(filename), "accountStateRestore.ts");
    const helper = new Module(helperPath, module);
    helper._compile(ts.transpileModule(fs.readFileSync(helperPath, "utf8"), {
      compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
    }).outputText, helperPath);
    return helper.exports;
  };
  loaded._compile(ts.transpileModule(fs.readFileSync(filename, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText, filename);
  return { ...loaded.exports, settings };
}

test("sign-out suppresses cloud work until auth has finished, then reloads", async () => {
  const { exitAccountMode, settings } = fixture();
  let resolveSignOut;
  let reloads = 0;
  const signOut = new Promise(resolve => { resolveSignOut = resolve; });
  const transition = exitAccountMode(false, () => signOut, () => { reloads++; });
  assert.equal(settings.get("dusk-account-mode"), "guest");
  assert.equal(reloads, 0);
  resolveSignOut();
  await transition;
  assert.equal(settings.has("dusk-account-mode"), false);
  assert.equal(reloads, 1);
});

test("failed sign-out restores account mode and leaves the current scope usable", async () => {
  const { exitAccountMode, settings } = fixture();
  let reloads = 0;
  await assert.rejects(exitAccountMode(false, async () => { throw new Error("offline"); }, () => { reloads++; }), /offline/);
  assert.equal(settings.has("dusk-account-mode"), false);
  assert.equal(reloads, 0);
});

test("guest sign-in reloads without signing out a retained session", async () => {
  const { exitAccountMode, settings } = fixture("guest");
  let reloads = 0;
  await exitAccountMode(true, async () => { assert.fail("guest sign-in should not sign out"); }, () => { reloads++; });
  assert.equal(settings.has("dusk-account-mode"), false);
  assert.equal(reloads, 1);
});

test("repeated clicks share one account transition", async () => {
  const { exitAccountMode } = fixture();
  let resolveSignOut;
  let calls = 0;
  const signOut = () => { calls++; return new Promise(resolve => { resolveSignOut = resolve; }); };
  const first = exitAccountMode(false, signOut, () => {});
  const second = exitAccountMode(false, signOut, () => {});
  assert.equal(calls, 1);
  resolveSignOut();
  await Promise.all([first, second]);
});
