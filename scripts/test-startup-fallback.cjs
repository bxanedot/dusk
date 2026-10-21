const assert = require("node:assert/strict");
const fs = require("node:fs");
const Module = require("node:module");
const path = require("node:path");
const ts = require("typescript");

const helperPath = path.resolve(__dirname, "../src/lib/startupFallback.ts");
const source = fs.readFileSync(helperPath, "utf8");
const output = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    esModuleInterop: true,
  },
}).outputText;
const helperModule = new Module(helperPath, module);
helperModule.filename = helperPath;
helperModule.paths = Module._nodeModulePaths(path.dirname(helperPath));
helperModule._compile(output, helperPath);
const { fallbackAccountProfile, withStartupFallback } = helperModule.exports;

async function run() {
  const user = {
    email: "listener@example.com",
    user_metadata: { username: "listener", display_name: "Local Name" },
  };
  assert.deepEqual(fallbackAccountProfile(user), {
    username: "listener",
    displayName: "Local Name",
  });

  assert.equal(
    await withStartupFallback(Promise.resolve("remote profile"), "local profile", 30),
    "remote profile",
  );
  assert.equal(
    await withStartupFallback(Promise.reject(new Error("offline")), "local profile", 30),
    "local profile",
  );
  assert.equal(
    await withStartupFallback(new Promise(() => {}), "local profile", 20),
    "local profile",
    "a stalled optional account request must not hold startup",
  );
  console.log("Startup account fallback checks passed.");
}

run().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
