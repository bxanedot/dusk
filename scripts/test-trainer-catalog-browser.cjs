const assert = require("node:assert/strict");
const fs = require("node:fs");
const Module = require("node:module");
const path = require("node:path");
const React = require("react");
const { renderToStaticMarkup } = require("react-dom/server");
const ts = require("typescript");

require.extensions[".tsx"] = (componentModule, filename) => {
  const componentSource = fs.readFileSync(filename, "utf8");
  const compiled = ts.transpileModule(componentSource, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      jsx: ts.JsxEmit.ReactJSX,
      esModuleInterop: true,
      allowSyntheticDefaultImports: true,
    },
  }).outputText;
  componentModule._compile(compiled, filename);
};

const componentPath = path.resolve(__dirname, "../src/components/TrainerCatalogBrowser.tsx");
assert.ok(fs.existsSync(componentPath), "full trainer catalog browser should be implemented");

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
const { TrainerCatalogBrowser, filterTrainerCatalogResults } = componentModule.exports;

const entries = [
  {
    title: "Elden Ring", platforms: ["Steam"],
    cheatCount: 12, status: "available",
  },
  {
    title: "The Witcher 3: Wild Hunt", platforms: ["GOG"],
    cheatCount: 8, status: "unsupported",
  },
  {
    title: "Baldur's Gate 3", platforms: ["Steam", "GOG"],
    cheatCount: null, status: "unavailable",
  },
];

assert.deepEqual(
  filterTrainerCatalogResults(entries, "ring elden", "all").map((entry) => entry.title),
  ["Elden Ring"],
  "catalog search should match title tokens regardless of order",
);
assert.deepEqual(
  filterTrainerCatalogResults(entries, "witcher wild hunt", "unsupported").map((entry) => entry.title),
  ["The Witcher 3: Wild Hunt"],
  "catalog filters should combine title matching with availability",
);
assert.deepEqual(
  filterTrainerCatalogResults(entries, "gate", "available"),
  [],
  "availability filters should hide entries with a different status",
);

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

function baseProps(overrides = {}) {
  return {
    state: "ready",
    results: entries,
    query: "",
    statusFilter: "all",
    page: 1,
    pageSize: 1,
    openingKey: null,
    errorMessage: null,
    onQueryChange: () => {},
    onStatusFilterChange: () => {},
    onRetry: () => {},
    onOpen: () => {},
    onPageChange: () => {},
    ...overrides,
  };
}

const filteredMarkup = renderToStaticMarkup(React.createElement(TrainerCatalogBrowser, baseProps({ query: "witcher" })));
assert.match(filteredMarkup, /The Witcher 3: Wild Hunt/);
assert.doesNotMatch(filteredMarkup, /Elden Ring/);
assert.match(filteredMarkup, /1 result/);

let nextPage = null;
const tree = TrainerCatalogBrowser(baseProps({ onPageChange: (page) => { nextPage = page; } }));
const nextButton = elementsOfType(tree, "button").find((button) => elementText(button).includes("Next page"));
assert.ok(nextButton, "catalog browser should provide next-page navigation");
nextButton.props.onClick();
assert.equal(nextPage, 2, "next-page navigation should advance the visible result window");

const errorMarkup = renderToStaticMarkup(React.createElement(TrainerCatalogBrowser, baseProps({
  state: "error",
  errorMessage: "Catalog unavailable.",
})));
assert.match(errorMarkup, /Catalog unavailable\./);
assert.match(errorMarkup, /Try again/);

console.log("Trainer catalog browser behavior checks passed.");
