const assert = require("node:assert/strict");
const fs = require("node:fs");
const Module = require("node:module");
const path = require("node:path");
const React = require("react");
const { renderToStaticMarkup } = require("react-dom/server");
const ts = require("typescript");

const componentPath = path.resolve(__dirname, "../src/components/TrainerCatalogPanel.tsx");
assert.ok(fs.existsSync(componentPath), "TrainerCatalogPanel should be implemented");

const source = fs.readFileSync(componentPath, "utf8");
const output = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    jsx: ts.JsxEmit.ReactJSX,
    esModuleInterop: true,
    allowSyntheticDefaultImports: true,
  },
}).outputText;
const componentModule = new Module(componentPath, module);
componentModule.filename = componentPath;
componentModule.paths = Module._nodeModulePaths(path.dirname(componentPath));
componentModule._compile(output, componentPath);
const { TrainerCatalogPanel } = componentModule.exports;

function elementsOfType(node, type, result = []) {
  if (!React.isValidElement(node)) return result;
  if (node.type === type) result.push(node);
  React.Children.forEach(node.props.children, (child) => elementsOfType(child, type, result));
  return result;
}

function elementText(node) {
  if (node == null || typeof node === "boolean") return "";
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(elementText).join("");
  if (React.isValidElement(node)) return elementText(node.props.children);
  return "";
}

function markup(props) {
  return renderToStaticMarkup(React.createElement(TrainerCatalogPanel, props));
}

function baseProps(overrides = {}) {
  return {
    gameTitle: "Example Game",
    state: "idle",
    results: [],
    openingKey: null,
    errorMessage: null,
    onSearch: () => {},
    onBrowse: () => {},
    onOpen: () => {},
    ...overrides,
  };
}

const availableResult = {
  title: "Example Game",
  titleId: "123",
  gameId: "456",
  platforms: ["Steam", "Xbox"],
  cheatCount: 42,
  updated: "Oct 12, 2026",
  status: "available",
};

let searchCalls = 0;
const idleTree = TrainerCatalogPanel(baseProps({ onSearch: () => { searchCalls += 1; } }));
const searchButton = elementsOfType(idleTree, "button").find(
  (button) => elementText(button).includes("Search catalog"),
);
assert.ok(searchButton, "idle panel should provide a Search catalog action");
searchButton.props.onClick();
assert.equal(searchCalls, 1, "Search catalog should invoke the supplied search action");

const loadingMarkup = markup(baseProps({ state: "loading" }));
assert.match(loadingMarkup, /Searching catalog/);
assert.match(loadingMarkup, /disabled/);

const errorMarkup = markup(baseProps({ state: "error", errorMessage: "Catalog request failed." }));
assert.match(errorMarkup, /Catalog request failed\./);
const emptyMarkup = markup(baseProps({ state: "empty" }));
assert.match(emptyMarkup, /Browse full catalog/);
assert.doesNotMatch(errorMarkup, /No trainer matches were found/);

let browseCalls = 0;
const emptyTree = TrainerCatalogPanel(baseProps({
  state: "empty",
  onBrowse: () => { browseCalls += 1; },
}));
const browseButton = elementsOfType(emptyTree, "button").find(
  (button) => elementText(button).includes("Browse full catalog"),
);
assert.ok(browseButton, "empty game lookup should offer the full catalog");
browseButton.props.onClick();
assert.equal(browseCalls, 1, "Browse full catalog should invoke the supplied navigation action");

const resultsMarkup = markup(baseProps({ state: "results", results: [availableResult] }));
assert.match(resultsMarkup, /Example Game/);
assert.match(resultsMarkup, /Available/);
assert.match(resultsMarkup, /Steam/);
assert.match(resultsMarkup, /Xbox/);
assert.match(resultsMarkup, /42 cheats/);
assert.match(resultsMarkup, /Oct 12, 2026/);
assert.match(resultsMarkup, /Trainer controls are handled by the trainer app\./);
assert.match(resultsMarkup, /Open trainer/);

let openedResult = null;
const availableTree = TrainerCatalogPanel(baseProps({
  state: "results",
  results: [availableResult],
  onOpen: (result) => { openedResult = result; },
}));
const openButton = elementsOfType(availableTree, "button").find(
  (button) => elementText(button).includes("Open trainer"),
);
assert.ok(openButton, "available result with IDs should offer Open trainer");
openButton.props.onClick();
assert.equal(openedResult, availableResult, "Open trainer should pass the selected result");

for (const result of [
  { ...availableResult, status: "unavailable" },
  { ...availableResult, status: "unsupported" },
  { ...availableResult, status: "unknown" },
  { ...availableResult, gameId: null },
  { ...availableResult, titleId: null },
  { ...availableResult, titleId: "0" },
  { ...availableResult, gameId: "18446744073709551616" },
  { ...availableResult, titleId: "12x" },
]) {
  const rendered = markup(baseProps({ state: "results", results: [result] }));
  assert.doesNotMatch(rendered, /Open trainer/);
}

const missingCountMarkup = markup(baseProps({
  state: "results",
  results: [{ ...availableResult, cheatCount: null, updated: null }],
}));
assert.doesNotMatch(missingCountMarkup, /0 cheats/);

console.log("Trainer catalog UI behavior checks passed.");
