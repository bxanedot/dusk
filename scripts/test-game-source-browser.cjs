const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const path = require("node:path");

const script = fs.readFileSync(
  path.join(__dirname, "..", "src-tauri", "src", "game_source_navigation.js"), "utf8",
);

function browser(url, userActivated = false) {
  const navigated = [];
  const events = new Map();
  const notes = [];
  const location = new URL(url);
  location.assign = (url) => navigated.push(url);

  class Element {
    constructor(href) { this.href = href; this.target = "_blank"; }
    closest() { return this; }
    getAttribute(key) { return key === "href" ? this.href : key === "target" ? this.target : null; }
    hasAttribute(key) { return key === "download" ? false : false; }
  }
  const window = { location, setTimeout() {}, clearTimeout() {} };
  window.top = window;
  window.open = () => null;
  const document = {
    body: { appendChild(node) { notes.push(node); } },
    addEventListener(name, cb) { events.set(name, cb); },
    getElementById() { return notes.find(node => node.id === "dusk-source-note") || null; },
    createElement() { return { style: {}, setAttribute() {}, remove() {} }; },
  };
  vm.runInNewContext(script, { URL, location, window, document, Element, navigator: { userActivation: { isActive: userActivated } } });
  const click = (href) => {
    let prevented = false;
    events.get("click")?.({
      target: new Element(href), button: 0, defaultPrevented: false,
      ctrlKey: false, altKey: false, shiftKey: false, metaKey: false,
      preventDefault() { prevented = true; }, stopImmediatePropagation() {},
    });
    return prevented;
  };
  return { window, navigated, notes, click };
}

const fitgirl = browser("https://fitgirl-repacks.site/game-title/");
assert.equal(fitgirl.click("https://fitgirl-repacks.site/other-game/"), true);
assert.equal(fitgirl.navigated[0], "https://fitgirl-repacks.site/other-game/");
fitgirl.window.open("https://www.fitgirl-repacks.site/?s=example", "_blank");
assert.equal(fitgirl.navigated.at(-1), "https://www.fitgirl-repacks.site/?s=example");
assert.equal(fitgirl.click("https://exoclick.com/ad"), true);
assert.equal(fitgirl.navigated.length, 2);
assert.equal(fitgirl.notes.length, 1);

const game3rb = browser("https://game3rb.com/game-title/");
assert.equal(game3rb.click("https://www.game3rb.com/next-game/"), true);
assert.equal(game3rb.navigated[0], "https://www.game3rb.com/next-game/");
assert.equal(game3rb.click("https://game3rb.com.evil.example/ad"), true);
assert.equal(game3rb.navigated.length, 1);

const approvedHost = browser("https://fitgirl-repacks.site/game/", true);
assert.equal(approvedHost.window.open("https://pixeldrain.com/u/abc", "_blank"), approvedHost.window);
assert.equal(approvedHost.navigated[0], "https://pixeldrain.com/u/abc");
approvedHost.window.open("https://ads.exoclick.com/pop", "_blank");
assert.equal(approvedHost.navigated.length, 1);

const unrelated = browser("https://unrelated.example/game/");
assert.equal(unrelated.click("https://game3rb.com/game/"), false);
assert.equal(unrelated.navigated.length, 0);

console.log("Game source browser navigation tests passed.");
