// Game3rb and FitGirl pages are unprivileged, isolated webviews.
// Follow trusted same-site target=_blank links in the current window,
// never grant the page access to Tauri's local game library.
(() => {
  "use strict";
  if (window !== window.top || location.protocol !== "https:") return;

  const domains = new Set(["game3rb.com", "fitgirl-repacks.site"]);
  const host = location.hostname.toLowerCase().replace(/^www\./, "");
  if (!domains.has(host)) return;

  const isSameSource = (value) => {
    if (typeof value !== "string" || !value.trim()) return null;
    try {
      const url = new URL(value, location.href);
      const destinationHost = url.hostname.toLowerCase().replace(/^www\./, "");
      return url.protocol === "https:" && destinationHost === host &&
        !url.username && !url.password ? url : null;
    } catch { return null; }
  };

  // Explicit download-host links can open in the current WebView.
  const fileHosts = new Set(["gofile.io","pixeldrain.com","mega.nz","1fichier.com","filecrypt.cc","filecrypt.co","rapidgator.net","multiup.io","qiwi.gg","datanodes.to","buzzheavier.com","vikingfile.com","filekeeper.net","fileditchfiles.st"]);
  function downloadHost(value) {
    try {
      const url = new URL(value, location.href);
      return url.protocol === "https:" && !url.username && !url.password && fileHosts.has(url.hostname.toLowerCase()) ? url : null;
    } catch { return null; }
  }
  let noticeTimer;
  function explainBlockedLink() {
    if (!document.body) return;
    let note = document.getElementById("dusk-source-note");
    if (!note) {
      note = document.createElement("div");
      note.id = "dusk-source-note";
      note.setAttribute("role", "status");
      note.style.cssText = "position:fixed;bottom:12px;left:12px;right:12px;z-index:2147483646;" +
        "padding:13px 16px;background:#16161c;color:#fff;border:1px solid #7960ac;" +
        "border-radius:10px;box-shadow:0 8px 32px #000b;font:13px sans-serif;";
      note.textContent = "Dusk blocked a third-party website or pop-up. To use an external download host, open the listing with Browser fallback.";
      document.body.appendChild(note);
    }
    window.clearTimeout(noticeTimer);
    noticeTimer = window.setTimeout(() => { note?.remove(); }, 9000);
  }

  const originalOpen = window.open.bind(window);
  window.open = function(target, name, features) {
    if (target === undefined || target === null || target === "") {
      // Sites sometimes call window.open() only to create advertising pop-ups.
      explainBlockedLink();
      return null;
    }
    const permitted = isSameSource(target);
    if (permitted) {
      window.location.assign(permitted.href);
      return window;
    }
    // JavaScript download buttons commonly call window.open during a real
    // user gesture. Only permit selected file hosts during that activation.
    const fileHost = downloadHost(target);
    if (fileHost && typeof navigator !== "undefined" && navigator.userActivation?.isActive) {
      window.location.assign(fileHost.href);
      return window;
    }
    explainBlockedLink();
    return null;
  };

  document.addEventListener("click", (event) => {
    if (event.defaultPrevented || event.button !== 0 ||
        event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) return;
    const element = event.target;
    if (!(element instanceof Element)) return;
    const anchor = element.closest('a[href]');
    if (!anchor || anchor.hasAttribute("download")) return;
    if (anchor.getAttribute("target") !== "_blank" && !downloadHost(anchor.getAttribute("href"))) return;
    const link = isSameSource(anchor.getAttribute("href"));
    event.preventDefault();
    event.stopImmediatePropagation();
    if (link) window.location.assign(link.href);
    else {
      const download = downloadHost(anchor.getAttribute("href"));
      if (download && event.isTrusted !== false) window.location.assign(download.href);
      else explainBlockedLink();
    }
  }, true);
})();
