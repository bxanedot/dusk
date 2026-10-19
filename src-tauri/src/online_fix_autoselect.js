// Runs only in Dusk's isolated Online-Fix download window.
// The injected source is confined to exact official file-host domains.
// It never invokes privileged Tauri APIs, interacts with ad tasks, or bypasses login.
(() => {
  "use strict";
  if (window !== window.top) return;
  const permittedHosts = new Set(["drive.online-fix.me", "hosters.online-fix.me"]);
  if (!permittedHosts.has(location.hostname) || location.protocol !== "https:") return;

  const gameTitle = __DUSK_TITLE__;
  const normalize = (text) => String(text || "").toLowerCase()
    .replace(/по\s+сети/gu, " ")
    .replace(/\b(online|multiplayer|coop|co-op)\b/g, " ")
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();
  const words = normalize(gameTitle).split(/\s+/).filter((word) => word.length >= 2);
  if (!words.length) return;

  const forbidden = new Set(["fix", "repair", "update", "updates", "patch", "crack", "cracks", "redist", "trainer", "cheat", "cheats"]);
  const archiveExt = /(?:\.part\d{1,4}\.rar|\.rar|\.7z(?:\.\d{3})?|\.zip)$/i;
  const advertisingHosts = [
    "exoclick.com", "exosrv.com", "magsrv.com", "realsrv.com",
    "juicyads.com", "adsterra.com", "popcash.net", "popads.net",
    "onclickads.net", "propellerads.com",
  ];
  const selected = new Set();
  const navigated = new Set();
  const maxFiles = 24;
  let started = 0;
  let lastNavigation = 0;

  function safeLink(anchor) {
    let url;
    try {
      url = new URL(anchor.getAttribute("href"), location.href);
    } catch {
      return null;
    }
    if (url.protocol !== "https:") return null;
    const blocked = advertisingHosts.some((name) =>
      url.hostname === name || url.hostname.endsWith("." + name));
    if (blocked) return null;

    const downloadName = anchor.getAttribute("download") || "";
    let path = url.pathname;
    try { path = decodeURIComponent(path); } catch {}
    const leaf = path.split("/").filter(Boolean).at(-1) || "";
    const label = String(anchor.textContent || anchor.getAttribute("title") || "").trim();
    const hintedName = url.searchParams.get("filename") || url.searchParams.get("file") || "";
    // Check each candidate independently; appending button text ("Download")
    // to ".rar" used to make extension checks incorrectly reject valid files.
    const isArchive = [leaf, downloadName, hintedName, label]
      .some((part) => archiveExt.test(part.trim()));
    const officialHost = permittedHosts.has(url.hostname);
    // Only direct archives can be followed onto unknown HTTPS CDN domains.
    if (!officialHost && !isArchive) return null;

    const candidate = [leaf, downloadName, hintedName, label].join(" ");
    const normalized = normalize(candidate);
    const pathnameWords = normalize(path).split(/\s+/);
    if (pathnameWords.some((word) => forbidden.has(word))) return null;
    const titleMatch = words.every((word) => normalized.split(/\s+/).includes(word));
    // Inside a verified game folder, split archives may have generic part
    // names; still require a file with an actual archive extension.
    let currentPath = location.pathname;
    try { currentPath = decodeURIComponent(currentPath); } catch {}
    const folderMatch = normalize(currentPath).includes(normalize(gameTitle));
    if (!titleMatch && !(isArchive && folderMatch)) return null;
    return { url, candidate, downloadName, isArchive };
  }

  function scan() {
    if (started >= maxFiles) return;
    const host = location.hostname;
    if (!permittedHosts.has(host)) return;
    const anchors = [...document.querySelectorAll("a[href]")];
    const files = [];
    const folders = [];
    for (const anchor of anchors) {
      const found = safeLink(anchor);
      if (!found) continue;
      if (anchor.closest('[class*="advert"],[id*="advert"],[class*="sponsor"]')) continue;
      if (found.isArchive) {
        if (!selected.has(found.url.href)) files.push({ anchor, ...found });
      } else {
        const text = String(anchor.textContent || "").toLowerCase();
        const leaf = found.url.pathname.split("/").filter(Boolean).pop() || "";
        const exactGameFolder = normalize(leaf) === normalize(gameTitle);
        const isGameFolder = /^(?:game|game files|full game|игра|файлы игры)$/i.test(text.trim());
        if ((exactGameFolder || isGameFolder) &&
            found.url.pathname !== location.pathname &&
            !navigated.has(found.url.href)) {
          folders.push({ anchor, ...found });
        }
      }
    }
    if (files.length) {
      // Start standard browser downloads. Avoid clicking advertisements and
      // skip anything not recognizably an archive for the selected game.
      files.sort((a, b) => a.url.href.localeCompare(b.url.href, undefined, { numeric: true }));
      const next = files[0];
      selected.add(next.url.href);
      started++;
      next.anchor.removeAttribute("target");
      next.anchor.click();
      return;
    }
    if (!started && folders.length && Date.now() - lastNavigation > 4000) {
      lastNavigation = Date.now();
      navigated.add(folders[0].url.href);
      folders[0].anchor.removeAttribute("target");
      folders[0].anchor.click();
    }
  }

  // Hosts can render a directory listing asynchronously after a user completes
  // authentication or a supported download gate. Keep checking only those links.
  function begin() {
    scan();
    const observer = new MutationObserver(() => { if (started < maxFiles) scan(); });
    observer.observe(document.documentElement, { subtree: true, childList: true });
    const interval = window.setInterval(scan, 1800);
    window.setTimeout(() => { observer.disconnect(); clearInterval(interval); }, 20 * 60 * 1000);
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", begin, { once: true });
  } else {
    begin();
  }
})();
