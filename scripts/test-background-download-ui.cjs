const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const root = path.join(__dirname, "..");
const app = fs.readFileSync(path.join(root, "src", "App.tsx"), "utf8");
assert.ok(!app.includes("Download game archives directly inside Dusk. Paste an actual HTTPS file URL"));
assert.ok(!app.includes('className="native-download-manager"'));
assert.ok(!app.includes("downloadManagerOpen"));
// The import now forwards source-specific extraction passwords on a separate line.
assert.match(app, /api\.importDownloadedGameArchive\(\s*ready\.filePath,\s*ready\.title,/);
const promptCalls = [...app.matchAll(/await finishGameImport\(imported(?:,\s*(true|false))?\)/g)];
assert.equal(promptCalls.length, 2, "Expected both background auto-import paths");
assert.ok(promptCalls.every((match) => match[1] === "true"),
  "Background imports must NEVER invoke a blocking native installer confirmation dialog");
assert.ok(app.includes('importingInBackgroundRef.current = true'));
assert.ok(app.includes('managedDownloadPathsRef.current.has(candidate.path.toLowerCase())'));
assert.ok(app.includes('api.openGameSourceListing(result.url)'));
assert.ok(app.includes('api.openGameSourceBrowser(result.url)'));

const navigation = fs.readFileSync(path.join(root, "src-tauri", "src", "game_source_navigation.js"), "utf8");
function browse(host) {
  const destinations = [];
  const callbacks = [];
  const location = new URL("https://" + host + "/game/");
  location.assign = (url) => destinations.push(url);
  class Element {
    constructor(link) { this.link = link; this.target = "_blank"; }
    closest() { return this; }
    hasAttribute(name) { return name === "download" ? false : false; }
    getAttribute(name) { return name === "href" ? this.link : name === "target" ? this.target : null; }
  }
  const window = {
    open() { return null; },
    setTimeout() { return 1; },
    clearTimeout() {},
    location,
  };
  window.top = window;
  const document = {
    body: { appendChild() {} },
    addEventListener(event, callback) { callbacks.push({ event, callback }); },
    getElementById() { return null; },
    createElement() {
      return { setAttribute() {}, style: {}, remove() {} };
    },
  };
  vm.runInNewContext(navigation, { location, window, document, Element, URL });
  const click = (url) => {
    let prevented = false;
    const event = {
      target: new Element(url), button: 0, defaultPrevented: false,
      ctrlKey: false, altKey: false, shiftKey: false, metaKey: false,
      preventDefault() { prevented = true; },
      stopImmediatePropagation() {},
    };
    callbacks.find((item) => item.event === "click")?.callback(event);
    return prevented;
  };
  return { click, destinations };
}
const fitgirl = browse("fitgirl-repacks.site");
assert.equal(fitgirl.click("https://fitgirl-repacks.site/another-game/"), true);
assert.equal(fitgirl.destinations[0], "https://fitgirl-repacks.site/another-game/");
assert.equal(fitgirl.click("https://ads.example/redirect"), true);
assert.equal(fitgirl.destinations.length, 1);
const game3rb = browse("game3rb.com");
assert.equal(game3rb.click("https://www.game3rb.com/another-game/"), true);
assert.equal(game3rb.destinations[0], "https://www.game3rb.com/another-game/");

const adblock = fs.readFileSync(path.join(root, "src-tauri", "src", "online_fix_adblock.js"), "utf8");
for (const domain of ["game3rb.com", "fitgirl-repacks.site"]) {
  assert.ok(adblock.includes('"' + domain + '"'), "Adblock missing " + domain);
}
console.log("Background download UI and source browser tests passed.");
