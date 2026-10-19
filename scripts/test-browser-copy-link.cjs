const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const path = require("node:path");

const script = fs.readFileSync(path.join(__dirname, "..", "src-tauri", "src", "browser_copy_link.js"), "utf8");

async function verify() {
  let href = "https://fitgirl-repacks.site/example-game/";
  const elements = new Map();
  const listeners = {};
  const copied = [];
  class Node {
    constructor(tag) {
      this.tagName = tag;
      this.style = {};
      this.isConnected = true;
      this.listeners = {};
      this.attrs = {};
    }
    set id(value) { this._id = value; elements.set(value, this); }
    get id() { return this._id; }
    setAttribute(key, value) { this.attrs[key] = value; }
    addEventListener(name, cb) { this.listeners[name] = cb; }
    appendChild(node) { node.isConnected = true; }
    remove() { this.isConnected = false; }
    focus() {}
    select() {}
  }
  const document = {
    readyState: "complete",
    body: new Node("body"),
    getElementById: id => elements.get(id) || null,
    createElement: tag => new Node(tag),
    addEventListener: (name, cb) => { listeners[name] = cb; },
    execCommand: () => false,
  };
  const location = { protocol: "https:", get href() { return href; } };
  const navigator = { clipboard: { writeText: async text => copied.push(text) } };
  const window = { setTimeout() {} };
  window.top = window;
  vm.runInNewContext(script, { document, location, navigator, window });
  const button = elements.get("dusk-copy-page-link");
  assert.ok(button, "Copy link button must appear in a supported website");
  assert.equal(button.textContent, "Copy link");
  href = "https://filekeeper.net/file/abc?token=x";
  button.listeners.click({ preventDefault() {}, stopPropagation() {} });
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(copied, [href], "Copy link should copy the current URL after navigation");
  assert.equal(button.textContent, "Copied!");

  // Never add a privileged control inside an insecure or nested document.
  const noButtonDocument = {
    ...document,
    getElementById() { return null; },
    createElement() { throw new Error("No button should be mounted"); },
  };
  vm.runInNewContext(script, {
    document: noButtonDocument, location: { protocol: "http:", href: "http://example.com" },
    navigator, window,
  });
  console.log("Embedded website Copy link button tests passed.");
}
verify().catch(error => { console.error(error); process.exitCode = 1; });
