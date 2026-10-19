// Navigate verified game-host links inside the isolated Dusk browser.
// WebView2 can silently ignore target="_blank" anchors in external webviews.
// Keep ordinary site confirmations (including dangerous-download warnings)
// intact, but prevent pop-under links to ad networks.
(() => {
  "use strict";
  if (window !== window.top || location.protocol !== "https:") return;
  const officialHosts = new Set([
    "online-fix.me", "www.online-fix.me",
    "hosters.online-fix.me", "drive.online-fix.me", "uploads.online-fix.me",
    "fileditchfiles.st", "filekeeper.net", "pixeldrain.com",
    "gofile.io", "vikingfile.com",
  ]);
  const ads = new Set([
    "exoclick.com", "exosrv.com", "magsrv.com", "realsrv.com",
    "juicyads.com", "adsterra.com", "popads.net", "popcash.net",
    "propellerads.com", "onclickads.net", "trafficjunky.net",
  ]);
  const pageHost = location.hostname.toLowerCase();
  if (!officialHosts.has(pageHost)) return;
  const isHost = (hostname, domains) => domains.has(hostname) ||
    [...domains].some((domain) => hostname.endsWith("." + domain));
  const archive = /\.(zip|rar|7z|7z\.\d{3})(?:$)/i;

  function destination(href) {
    if (typeof href !== "string" || !href.trim()) return null;
    try {
      const url = new URL(href, location.href);
      if (url.protocol !== "https:" || isHost(url.hostname, ads)
          || url.username || url.password) return null;
      const trustedHost = officialHosts.has(url.hostname);
      const directArchive = archive.test(url.pathname);
      return trustedHost || directArchive ? url : null;
    } catch { return null; }
  }

  function notice() {
    if (document.getElementById("dusk-navigation-note")) return;
    const banner = document.createElement("div");
    banner.id = "dusk-navigation-note";
    banner.setAttribute("role", "status");
    banner.style.cssText = "position:fixed;bottom:10px;left:10px;right:10px;z-index:2147483647;" +
      "background:#17141f;color:#fff;padding:12px 16px;border:1px solid #6643a1;" +
      "border-radius:10px;font:13px sans-serif;box-shadow:0 4px 20px #000a;";
    banner.textContent = "Dusk blocked an unsupported link. Try a different file host or use Browser fallback in the launcher.";
    (document.body || document.documentElement).appendChild(banner);
    window.setTimeout(() => banner.remove(), 8500);
  }

  const normalOpen = window.open.bind(window);
  window.open = function(url, target, features) {
    const parsed = destination(url);
    if (parsed) {
      window.location.assign(parsed.href);
      return window;
    }
    // Refuse known popups and give a visible explanation for unsupported hosts.
    if (typeof url === "string" && url.trim()) { notice(); return null; }
    return normalOpen(url, target, features);
  };

  document.addEventListener("click", (event) => {
    if (event.defaultPrevented || event.button !== 0 ||
        event.ctrlKey || event.shiftKey || event.altKey || event.metaKey) return;
    const element = event.target;
    if (!(element instanceof Element)) return;
    const anchor = element.closest('a[href][target="_blank"]');
    if (!anchor || anchor.hasAttribute("download")) return;
    // Let the host show its warning modal first; only the human can confirm.
    if (anchor.dataset.dangerous === "true") return;
    const parsed = destination(anchor.getAttribute("href"));
    event.preventDefault();
    event.stopImmediatePropagation();
    if (parsed) {
      window.location.assign(parsed.href);
    } else {
      notice();
    }
  }, true);
})();
