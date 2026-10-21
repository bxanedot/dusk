const assert = require("node:assert/strict");
const fs = require("node:fs");
const Module = require("node:module");
const path = require("node:path");
const { test } = require("node:test");
const ts = require("typescript");

function loadSource(name, dependencies = {}) {
  const filename = path.resolve(__dirname, `../src/lib/${name}.ts`);
  const loaded = new Module(filename, module);
  loaded.filename = filename;
  loaded.require = request => dependencies[request] ?? module.require(request);
  loaded._compile(ts.transpileModule(fs.readFileSync(filename, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText, filename);
  return loaded.exports;
}

const { preservePendingAccountEdits: merge } = loadSource("accountStateRestore");
const nextTurn = () => new Promise(resolve => setImmediate(resolve));
function deferred() {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
}

test("unchanged fields take cloud values while new local records survive", () => {
  const baseline = { profiles: [{ id: "p", name: "Before" }] };
  const local = { profiles: [...baseline.profiles, { id: "new", name: "Created while offline" }] };
  const cloud = { profiles: [{ id: "p", name: "Remote edit" }, { id: "remote", name: "Other device" }] };
  assert.deepEqual(merge(cloud, baseline, local).profiles, [
    { id: "p", name: "Remote edit" }, { id: "remote", name: "Other device" }, local.profiles[1],
  ]);
});

test("local removals do not resurrect profiles or their remote memberships", () => {
  const baseline = {
    profiles: [{ id: "keep" }, { id: "removed" }],
    profileGames: [{ profileId: "keep", gameId: "removed-link" }],
    collections: [{ id: "removed-collection" }],
    collectionGames: [{ collectionId: "removed-collection", gameId: "game" }],
  };
  const local = { profiles: [{ id: "keep" }], profileGames: [], collections: [], collectionGames: [] };
  const cloud = {
    ...baseline,
    profileGames: [...baseline.profileGames, { profileId: "removed", gameId: "new-remote-game" }],
    collectionGames: [...baseline.collectionGames, { collectionId: "removed-collection", gameId: "new-remote-game" }],
  };
  const result = merge(cloud, baseline, local);
  assert.deepEqual(result.profiles, local.profiles);
  assert.deepEqual(result.profileGames, []);
  assert.deepEqual(result.collections, []);
  assert.deepEqual(result.collectionGames, []);
});

test("field merging preserves false values and does not modify input snapshots", () => {
  const baseline = { games: [{ id: "g", favorite: true, title: "Before" }] };
  const local = { games: [{ id: "g", favorite: false, title: "Before" }] };
  const cloud = { games: [{ id: "g", favorite: true, title: "Cloud title" }] };
  assert.deepEqual(merge(cloud, baseline, local).games, [{ id: "g", favorite: false, title: "Cloud title" }]);
  assert.equal(cloud.games[0].favorite, true);
  assert.equal(local.games[0].title, "Before");
});

test("mutations wait for the final snapshot import while library reads remain usable", async () => {
  const helper = loadSource("accountStateRestore");
  const calls = [];
  const { api } = loadSource("api", {
    "./accountStateRestore": helper,
    "@tauri-apps/api/core": { invoke: async command => { calls.push(command); return []; } },
  });
  const imported = deferred();
  const restoring = helper.withAccountStateLock(async () => {
    calls.push("import-start");
    await imported.promise;
    calls.push("import-end");
  });
  const editing = api.renameProfile("p", "New name");
  await api.listGames();
  await nextTurn();
  assert.ok(calls.includes("list_games"));
  assert.ok(!calls.includes("rename_profile"));
  imported.resolve();
  await Promise.all([restoring, editing]);
  assert.ok(calls.indexOf("rename_profile") > calls.indexOf("import-end"));
});

test("snapshot reconciliation waits for an already pending native edit", async () => {
  const helper = loadSource("accountStateRestore");
  const edited = deferred();
  const calls = [];
  const { api } = loadSource("api", {
    "./accountStateRestore": helper,
    "@tauri-apps/api/core": { invoke: async () => { calls.push("edit-start"); await edited.promise; calls.push("edit-end"); } },
  });
  const editing = api.setFavorite("game", true);
  const restoring = helper.withAccountStateLock(async () => { calls.push("snapshot-read"); });
  await nextTurn();
  assert.deepEqual(calls, ["edit-start"]);
  edited.resolve();
  await Promise.all([editing, restoring]);
  assert.deepEqual(calls, ["edit-start", "edit-end", "snapshot-read"]);
});

test("a failed native operation releases the state lock", async () => {
  const helper = loadSource("accountStateRestore");
  await assert.rejects(helper.withAccountStateLock(async () => { throw new Error("native failure"); }), /native failure/);
  assert.equal(await helper.withAccountStateLock(async () => "next edit"), "next edit");
});

test("restoration retains the largest legacy progress totals from either device", () => {
  const baseline = { games: [{ id: "g", totalSeconds: 10, launchCount: 1 }] };
  const local = { games: [{ id: "g", totalSeconds: 20, launchCount: 2 }] };
  const cloud = { games: [{ id: "g", totalSeconds: 100, launchCount: 8 }] };
  assert.deepEqual(merge(cloud, baseline, local).games, cloud.games);
  assert.deepEqual(merge(local, baseline, cloud).games, cloud.games);
});

test("long scans and archive extraction do not block library actions", async () => {
  for (const operation of ["scanGames", "scanPlatformAchievements", "importGameArchive", "importDownloadedGameArchive"]) {
    const helper = loadSource("accountStateRestore");
    const pending = deferred();
    const calls = [];
    const { api } = loadSource("api", {
      "./accountStateRestore": helper,
      "@tauri-apps/api/core": { invoke: command => {
        calls.push(command);
        return command === "set_favorite" ? Promise.resolve() : pending.promise;
      } },
    });
    const running = api[operation]("fixture", "Fixture");
    const editing = api.setFavorite("game", true);
    await nextTurn();
    assert.ok(calls.includes("set_favorite"), `${operation} held a long-lived mutation lock`);
    pending.resolve();
    await Promise.all([running, editing]);
  }
});
