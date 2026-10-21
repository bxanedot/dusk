const assert = require("node:assert/strict");
const fs = require("node:fs");
const Module = require("node:module");
const path = require("node:path");
const { test } = require("node:test");
const ts = require("typescript");

const filename = path.resolve(__dirname, "../src/lib/cloudSaves.ts");
const compiled = ts.transpileModule(fs.readFileSync(filename, "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText;

function fixture({ mode, session } = {}) {
  const settings = new Map(mode ? [["dusk-account-mode", mode]] : []);
  global.localStorage = { getItem: key => settings.get(key) ?? null };
  const calls = [];
  const identity = { data: { session: { user: { id: "signed-in-user" }, access_token: "fixture-token" } }, error: null };
  const client = { auth: { getSession: async () => {
    calls.push("get-session");
    return session ? session() : identity;
  } } };
  const api = Object.fromEntries([
    "uploadSaveBackupToCloud", "uploadCloudManifest", "uploadAllSaveBackupsToCloud",
  ].map(name => [name, async request => { calls.push({ name, request }); }]));
  const loaded = new Module(filename, module);
  loaded.filename = filename;
  loaded.paths = Module._nodeModulePaths(path.dirname(filename));
  loaded.require = request => {
    if (request === "./api") return { api };
    if (request === "./auth") return {
      getSupabaseClient: () => client,
      supabaseConfigured: () => true,
      SUPABASE_URL: "https://fixture.invalid",
      SUPABASE_KEY: "fixture-publishable-key",
    };
    return module.require(request);
  };
  loaded._compile(compiled, filename);
  return { ...loaded.exports, calls, settings, identity };
}

test("guest cloud-save status ignores a persisted signed-in session", async () => {
  const saves = fixture({ mode: "guest" });
  const status = await saves.cloudSaveStatus();
  assert.equal(status.authenticated, false);
  assert.equal(status.userId, null);
  assert.deepEqual(saves.calls, []);
});

test("guest backup and manifest uploads cannot use a previous account", async () => {
  const saves = fixture({ mode: "guest" });
  await assert.rejects(saves.syncBackupToSupabase({ id: "guest-backup" }), /sign in/i);
  await assert.rejects(saves.syncCloudManifest(), /sign in/i);
  await assert.rejects(saves.syncAllBackupsToSupabase(), /sign in/i);
  assert.deepEqual(saves.calls, []);
});

test("a pending cloud identity cannot upload after switching to guest", async () => {
  let completeSession;
  const session = new Promise(resolve => { completeSession = resolve; });
  const saves = fixture({ session: () => session });
  const uploading = saves.syncCloudManifest();
  saves.settings.set("dusk-account-mode", "guest");
  completeSession(saves.identity);
  await assert.rejects(uploading, /sign in/i);
  assert.deepEqual(saves.calls, ["get-session"]);
});

test("signed-in backups retain the selected account identity", async () => {
  const saves = fixture();
  await saves.syncBackupToSupabase({ id: "account-backup" });
  assert.equal(saves.calls[1].request.backupId, "account-backup");
  assert.equal(saves.calls[1].request.authUserId, "signed-in-user");
});
