const assert = require("node:assert/strict");
const fs = require("node:fs");
const Module = require("node:module");
const path = require("node:path");
const { test } = require("node:test");
const ts = require("typescript");

const filename = path.resolve(__dirname, "../src/lib/accountSync.ts");
const compiled = ts.transpileModule(fs.readFileSync(filename, "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText;
const remoteState = { profiles: [{ id: "remote-profile", name: "Remote library" }] };
const nextTurn = () => new Promise(resolve => setImmediate(resolve));

function deferred() {
  let resolve;
  const promise = new Promise(complete => { resolve = complete; });
  return { promise, resolve };
}

function fixture({ mode, read, importState, rpc, exportState, session } = {}) {
  const settings = new Map(mode ? [["dusk-account-mode", mode]] : []);
  global.localStorage = { getItem: key => settings.get(key) ?? null };
  global.window = new EventTarget();
  const calls = [];
  const api = {
    exportPlaytimeUpdates: async () => ({ baselines: [], sessions: [] }),
    mergeCloudPlaytime: async () => { calls.push("merge-playtime"); },
    exportAccountState: async () => exportState ? structuredClone(exportState()) : ({ profiles: [{ id: "local-profile" }] }),
    importAccountState: async state => {
      calls.push("import-start");
      await importState?.(state);
      calls.push("import-done");
    },
  };
  const client = {
    auth: { getSession: async () => session ? session() : ({ data: { session: { user: { id: "fixture-user" } } }, error: null }) },
    from: table => ({
      select: () => ({ eq: () => ({ maybeSingle: async () => {
        calls.push("read-cloud");
        return read ? read() : { data: { state: remoteState }, error: null };
      } }) }),
      upsert: async () => { calls.push(`upload:${table}`); return { error: null }; },
    }),
    rpc: async () => rpc ? rpc() : { data: [], error: null },
  };
  const loaded = new Module(filename, module);
  loaded.filename = filename;
  loaded.paths = Module._nodeModulePaths(path.dirname(filename));
  loaded.require = request => {
    if (request === "./api") return { api };
    if (request === "./auth") return { getSupabaseClient: () => { calls.push("session-client"); return client; } };
    if (request === "./accountStateRestore") {
      const helperPath = path.resolve(path.dirname(filename), "accountStateRestore.ts");
      const helper = new Module(helperPath, module);
      helper.filename = helperPath;
      helper._compile(ts.transpileModule(fs.readFileSync(helperPath, "utf8"), {
        compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
      }).outputText, helperPath);
      return helper.exports;
    }
    return module.require(request);
  };
  loaded._compile(compiled, filename);
  return { ...loaded.exports, calls, settings };
}

test("guest mode never reads or uploads a cached signed-in account", async () => {
  const sync = fixture({ mode: "guest" });
  await sync.syncAccountState();
  await sync.hydrateAccountState();
  assert.deepEqual(sync.calls, []);
});

test("startup uploads wait for both the cloud read and native state import", async () => {
  const cloud = deferred();
  const imported = deferred();
  const sync = fixture({
    read: () => cloud.promise,
    importState: () => imported.promise,
  });
  const restoring = sync.hydrateAccountState();
  const uploading = sync.syncAccountState();
  await nextTurn();
  assert.ok(!sync.calls.some(call => call.startsWith("upload:")), "local state overwrote the pending cloud restore");
  cloud.resolve({ data: { state: remoteState }, error: null });
  await nextTurn();
  assert.ok(!sync.calls.some(call => call.startsWith("upload:")), "upload raced the native import");
  imported.resolve();
  await Promise.all([restoring, uploading]);
  assert.ok(sync.calls.includes("import-done"));
});

test("the first periodic sync restores cloud state before uploading local state", async () => {
  const sync = fixture();
  await sync.syncAccountState();
  assert.ok(sync.calls.includes("read-cloud"));
  assert.ok(sync.calls.includes("import-done"));
  const upload = sync.calls.findIndex(call => call.startsWith("upload:"));
  assert.ok(upload === -1 || upload > sync.calls.indexOf("import-done"));
});

test("a failed initial restore must be retried before any later upload", async () => {
  const sync = fixture({ read: async () => ({ data: null, error: new Error("offline") }) });
  await assert.rejects(sync.hydrateAccountState(), /offline/);
  await assert.rejects(sync.syncAccountState(), /offline/);
  assert.equal(sync.calls.filter(call => call === "read-cloud").length, 2);
  assert.ok(!sync.calls.some(call => call.startsWith("upload:")));
});

test("an account with no remote snapshot uploads its local library without deadlocking", { timeout: 1000 }, async () => {
  const sync = fixture({ read: async () => ({ data: null, error: null }) });
  await sync.syncAccountState();
  assert.equal(sync.calls.filter(call => call === "read-cloud").length, 1);
  assert.equal(sync.calls.filter(call => call === "upload:dusk_account_state").length, 1);
});

test("concurrent restores share one cloud read", async () => {
  const cloud = deferred();
  const sync = fixture({ read: () => cloud.promise });
  const first = sync.hydrateAccountState();
  const second = sync.hydrateAccountState();
  await nextTurn();
  assert.equal(sync.calls.filter(call => call === "read-cloud").length, 1);
  cloud.resolve({ data: { state: remoteState }, error: null });
  await Promise.all([first, second]);
});

test("switching to guest during a cloud read prevents importing into the guest library", async () => {
  const cloud = deferred();
  const sync = fixture({ read: () => cloud.promise });
  const restoring = sync.hydrateAccountState();
  await nextTurn();
  sync.settings.set("dusk-account-mode", "guest");
  cloud.resolve({ data: { state: remoteState }, error: null });
  await restoring;
  assert.ok(!sync.calls.includes("import-start"));
  assert.ok(!sync.calls.some(call => call.startsWith("upload:")));
});

test("restored native state notifies the mounted library to refresh", async () => {
  const sync = fixture();
  window.addEventListener("dusk-account-state-hydrated", () => sync.calls.push("library-refresh"));
  await sync.hydrateAccountState();
  assert.ok(sync.calls.indexOf("library-refresh") > sync.calls.indexOf("import-done"));
});

test("a late playtime response cannot merge signed-in totals into guest data", async () => {
  const totals = deferred();
  const sync = fixture({ rpc: () => totals.promise });
  const restoring = sync.hydrateAccountState();
  await nextTurn();
  sync.settings.set("dusk-account-mode", "guest");
  totals.resolve({ data: [{ game_id: "fixture-game", total_seconds: 100 }], error: null });
  await restoring;
  assert.ok(!sync.calls.includes("merge-playtime"));
});

test("playtime retries never reimport a snapshot that already restored successfully", async () => {
  let rpcCount = 0;
  const sync = fixture({ rpc: async () => ++rpcCount === 1
    ? { data: null, error: new Error("playtime offline") }
    : { data: [], error: null } });
  await assert.rejects(sync.hydrateAccountState(), /playtime offline/);
  await sync.syncAccountState();
  assert.equal(sync.calls.filter(call => call === "import-done").length, 1,
    "a stale snapshot was imported again after local editing resumed");
  assert.ok(sync.calls.includes("upload:dusk_account_state"));
});

test("the library refreshes again when delayed hydration playtime finishes merging", async () => {
  const totals = deferred();
  const sync = fixture({ rpc: () => totals.promise });
  window.addEventListener("dusk-account-state-hydrated", () => sync.calls.push("library-refresh"));
  const restoring = sync.hydrateAccountState();
  await nextTurn();
  assert.equal(sync.calls.filter(call => call === "library-refresh").length, 1);
  totals.resolve({ data: [{ game_id: "fixture-game", total_seconds: 900 }], error: null });
  await restoring;
  assert.ok(sync.calls.lastIndexOf("library-refresh") > sync.calls.indexOf("merge-playtime"));
});

test("edits made while downloading survive cloud restoration", async () => {
  const cloud = deferred();
  let localState = {
    profiles: [{ id: "remote-profile", name: "Old local name" }],
    games: [{ id: "game", title: "Local title", favorite: false }],
    activeProfileId: "remote-profile",
  };
  const sync = fixture({
    read: () => cloud.promise,
    exportState: () => localState,
    importState: state => { localState = state; },
  });
  const restoring = sync.hydrateAccountState();
  await nextTurn();
  localState.profiles[0].name = "New local name";
  localState.games[0].favorite = true;
  localState.activeProfileId = "new-local-profile";
  cloud.resolve({ data: { state: {
    profiles: [{ id: "remote-profile", name: "Old remote name" }],
    games: [{ id: "game", title: "Updated cloud title", favorite: false }],
    activeProfileId: "remote-profile",
  } }, error: null });
  await restoring;
  assert.equal(localState.profiles[0].name, "New local name");
  assert.equal(localState.games[0].favorite, true);
  assert.equal(localState.games[0].title, "Updated cloud title");
  assert.equal(localState.activeProfileId, "new-local-profile");
});

test("edits after a failed download remain protected when the read is retried", async () => {
  let attempts = 0;
  let localState = { profiles: [{ id: "remote-profile", name: "Before" }] };
  const sync = fixture({
    read: async () => ++attempts === 1 ? { data: null, error: new Error("offline") }
      : { data: { state: remoteState }, error: null },
    exportState: () => localState,
    importState: state => { localState = state; },
  });
  await assert.rejects(sync.hydrateAccountState(), /offline/);
  localState.profiles[0].name = "Edited offline";
  await sync.syncAccountState();
  assert.equal(localState.profiles[0].name, "Edited offline");
});

test("uploads wait for active hydration even after its snapshot import commits", async () => {
  const totals = deferred();
  const sync = fixture({ rpc: () => totals.promise });
  const restoring = sync.hydrateAccountState();
  await nextTurn();
  assert.ok(sync.calls.includes("import-done"));
  const uploading = sync.syncAccountState();
  await nextTurn();
  assert.ok(!sync.calls.some(call => call.startsWith("upload:")));
  totals.resolve({ data: [], error: null });
  await Promise.all([restoring, uploading]);
});

test("edits made while authentication is pending survive restoration", async () => {
  const auth = deferred();
  let localState = { profiles: [{ id: "remote-profile", name: "Before authentication" }] };
  const sync = fixture({
    session: () => auth.promise,
    exportState: () => localState,
    importState: state => { localState = state; },
  });
  const restoring = sync.hydrateAccountState();
  await nextTurn();
  localState.profiles[0].name = "Edited during authentication";
  auth.resolve({ data: { session: { user: { id: "fixture-user" } } }, error: null });
  await restoring;
  assert.equal(localState.profiles[0].name, "Edited during authentication");
});

test("an authentication retry retains the baseline captured before offline edits", async () => {
  let attempts = 0;
  let localState = { profiles: [{ id: "remote-profile", name: "Before authentication" }] };
  const sync = fixture({
    session: async () => ++attempts === 1
      ? { data: { session: null }, error: new Error("authentication offline") }
      : { data: { session: { user: { id: "fixture-user" } } }, error: null },
    exportState: () => localState,
    importState: state => { localState = state; },
  });
  await assert.rejects(sync.hydrateAccountState(), /authentication offline/);
  localState.profiles[0].name = "Edited while offline";
  await sync.hydrateAccountState();
  assert.equal(localState.profiles[0].name, "Edited while offline");
});
