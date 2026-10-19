// Lightweight, isolated webview toolbar. No Tauri API is exposed to websites.
// Copy the CURRENT URL rather than the original listing (redirects/navigation).
(() => {
  "use strict";
  if (window !== window.top || location.protocol !== "https:") return;
  const toolbarId = "dusk-copy-page-link";

  function mount() {
    if (!document.body || document.getElementById(toolbarId)) return;
    const button = document.createElement("button");
    button.id = toolbarId;
    button.type = "button";
    button.textContent = "Copy link";
    button.title = "Copy the current website address";
    button.setAttribute("aria-label", "Copy current website link");
    button.style.cssText = [
      "all:initial", "box-sizing:border-box", "position:fixed", "top:14px", "right:14px",
      "z-index:2147483645", "display:inline-flex", "align-items:center", "justify-content:center",
      "padding:9px 13px", "border-radius:9px", "border:1px solid #69637c",
      "background:#15131c", "color:#fff", "font:600 12px/1.4 system-ui,sans-serif",
      "box-shadow:0 4px 16px #0007", "cursor:pointer", "user-select:none"
    ].join(";");
    const original = "Copy link";
    async function copyCurrentUrl() {
      const href = location.href;
      if (!/^https:\/\//i.test(href)) return;
      // First try the synchronous clipboard command while the click gesture
      // is still active. Then use the asynchronous Clipboard API if needed.
      let success = false;
      const input = document.createElement("textarea");
      input.value = href;
      input.setAttribute("aria-hidden", "true");
      input.style.cssText = "position:fixed;opacity:0;pointer-events:none;left:-9999px;";
      document.body.appendChild(input);
      input.focus();
      input.select();
      try { success = !!document.execCommand("copy"); } catch { success = false; }
      input.remove();
      if (!success) {
        try {
          if (navigator.clipboard && typeof navigator.clipboard.writeText === "function") {
            await navigator.clipboard.writeText(href);
            success = true;
          }
        } catch { success = false; }
      }
      button.textContent = success ? "Copied!" : "Copy failed";
      button.setAttribute("aria-label", success ? "Link copied" : "Could not copy link");
      window.setTimeout(() => {
        if (!button.isConnected) return;
        button.textContent = original;
        button.setAttribute("aria-label", "Copy current website link");
      }, 1800);
    }
    button.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      void copyCurrentUrl();
    });
    document.body.appendChild(button);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", mount, { once: true });
  } else {
    mount();
  }
})();
