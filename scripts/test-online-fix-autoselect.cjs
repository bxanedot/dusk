const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const source = fs.readFileSync(path.join(__dirname, "..", "src-tauri", "src", "online_fix_autoselect.js"), "utf8");

function tryPage(gameTitle, pageUrl, anchors) {
  const clicks = [];
  const location = new URL(pageUrl);
  class MutationObserver {
    observe() {}
    disconnect() {}
  }
  const links = anchors.map((entry) => ({
    getAttribute(key) {
      if (key === "href") return entry.href;
      if (key === "download") return entry.download || "";
      if (key === "title") return entry.title || "";
      return null;
    },
    closest() { return null; },
    removeAttribute() {},
    click() { clicks.push(entry.href); },
    textContent: entry.text || "Download",
  }));
  const page = {
    readyState: "complete",
    documentElement: {},
    querySelectorAll(selector) { return selector === "a[href]" ? links : []; },
    addEventListener() {},
  };
  const window = { setInterval() {}, setTimeout() {} };
  window.top = window;
  vm.runInNewContext(
    source.replace("__DUSK_TITLE__", JSON.stringify(gameTitle)),
    { document: page, location, window, MutationObserver, URL, decodeURIComponent },
  );
  return clicks;
}

const root = "https://drive.online-fix.me:2053/How%20to%20Fish/";
assert.deepEqual(
  tryPage("How to Fish по сети", root, [{
    href: "https://drive.online-fix.me:2053/How%20to%20Fish/How%20to%20Fish.part01.rar",
    text: "Download",
  }]),
  ["https://drive.online-fix.me:2053/How%20to%20Fish/How%20to%20Fish.part01.rar"],
);
assert.deepEqual(
  tryPage("How to Fish по сети", root, [{
    href: "https://cdn-files.example.net/Game/How%20to%20Fish.zip?signature=12345",
    text: "Download",
  }]),
  ["https://cdn-files.example.net/Game/How%20to%20Fish.zip?signature=12345"],
);
assert.deepEqual(
  tryPage("How to Fish по сети", root, [{
    href: "https://exoclick.com/How%20to%20Fish.zip",
    text: "Download",
  }]),
  [],
);
assert.deepEqual(
  tryPage("How to Fish по сети", root, [{
    href: "https://drive.online-fix.me:2053/How%20to%20Fish/Fix_Repair.zip",
    text: "Download",
  }]),
  [],
);
console.log("Online-Fix file selection tests passed.");
