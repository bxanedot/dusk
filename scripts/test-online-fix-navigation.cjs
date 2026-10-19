const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const path = require("node:path");

const script = fs.readFileSync(
  path.join(__dirname, "..", "src-tauri", "src", "online_fix_navigation.js"),
  "utf8",
);

function createWebview(initialUrl) {
  const navigated = [];
  const listeners = [];
  const banners = [];
  const location = new URL(initialUrl);
  location.assign = (url) => navigated.push(url);
  class Element {
    constructor(href, dangerous = false) {
      this.href = href;
      this.dataset = { dangerous: dangerous ? "true" : "false" };
    }
    closest(query) { return query.includes("target") ? this : null; }
    hasAttribute(name) { return name === "download" ? false : false; }
    getAttribute(name) { return name === "href" ? this.href : null; }
  }
  const window = { location, setTimeout() {} };
  window.top = window;
  window.open = () => null;
  const document = {
    body: { appendChild(banner) { banners.push(banner); } },
    addEventListener(event, callback) { listeners.push({ event, callback }); },
    getElementById() { return banners.find((item) => item.id === "dusk-navigation-note") || null; },
    createElement() { return { id: null, style: {}, setAttribute() {}, remove() {} }; },
  };
  vm.runInNewContext(script, { window, location, document, Element, URL });
  return { window, Element, navigated, listeners, banners };
}

function click(model, href, dangerous = false) {
  const event = {
    target: new model.Element(href, dangerous), button: 0, defaultPrevented: false,
    ctrlKey: false, shiftKey: false, altKey: false, metaKey: false,
    preventDefault() { this.defaultPrevented = true; },
    stopImmediatePropagation() {},
  };
  for (const { event: name, callback } of model.listeners) {
    if (name === "click") callback(event);
  }
  return event.defaultPrevented;
}

const listing = createWebview("https://hosters.online-fix.me:2053/How%20to%20Fish");
assert.equal(click(listing, "https://fileditchfiles.st/f/Game.rar"), true);
assert.equal(listing.navigated.at(-1), "https://fileditchfiles.st/f/Game.rar");
assert.equal(click(listing, "https://pixeldrain.com/u/abcd"), true);
assert.equal(listing.navigated.at(-1), "https://pixeldrain.com/u/abcd");
assert.equal(click(listing, "https://filekeeper.net/abcdef"), true);
assert.equal(listing.navigated.at(-1), "https://filekeeper.net/abcdef");

// Human must confirm the source site's warning; never auto-accept it.
const prior = listing.navigated.length;
assert.equal(click(listing, "https://vikingfile.com/f/xyz", true), false);
assert.equal(listing.navigated.length, prior);
listing.window.open("https://vikingfile.com/f/xyz", "_blank");
assert.equal(listing.navigated.at(-1), "https://vikingfile.com/f/xyz");

assert.equal(click(listing, "https://exoclick.com/ads"), true);
assert.equal(listing.banners.length, 1);
assert.equal(click(listing, "https://unknown-ads.example/pop"), true);
assert.equal(listing.banners.length, 1);

const other = createWebview("https://unrelated.example/");
assert.equal(click(other, "https://pixeldrain.com/u/abcd"), false);
assert.equal(other.navigated.length, 0);
console.log("Online-Fix navigation tests passed.");
