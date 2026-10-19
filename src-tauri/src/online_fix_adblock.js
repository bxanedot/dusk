// A lightweight content filter always enabled in Dusk's isolated game browsing windows.
// It is NOT a global browser extension or a replacement for a network filter.
// The containing Rust webview also blocks navigation and unsolicited pop-ups.
(() => {
  "use strict";
  if (window !== window.top || location.protocol !== "https:") return;
  const allowedPages = new Set([
    "online-fix.me", "www.online-fix.me", "hosters.online-fix.me",
    "drive.online-fix.me", "uploads.online-fix.me",
    "fileditchfiles.st", "filekeeper.net", "pixeldrain.com",
    "gofile.io", "vikingfile.com",
    "game3rb.com", "www.game3rb.com",
    "fitgirl-repacks.site", "www.fitgirl-repacks.site",
  ]);
  if (!allowedPages.has(location.hostname)) return;

  const advertisingHosts = [
    "acscdn.com", "themoneytizer.com", "adsterra.com",
    "propellerads.com", "onclickads.net", "popcash.net", "popads.net",
    "exoclick.com", "exosrv.com", "magsrv.com", "realsrv.com",
    "trafficjunky.net", "juicyads.com", "clickadu.com", "hilltopads.net",
    "adnxs.com", "doubleclick.net", "googlesyndication.com",
    "adservice.google.com", "adskeeper.co.uk", "ad-maven.com",
    "revenuehits.com", "adcash.com", "a-ads.com",
  ];
  function isAdvertisingUrl(value) {
    if (!value || typeof value !== "string") return false;
    try {
      const url = new URL(value, location.href);
      const host = url.hostname.toLowerCase();
      return advertisingHosts.some(domain => host === domain || host.endsWith("." + domain));
    } catch { return false; }
  }

  // Keep page controls and real download links intact. Limit hiding to
  // recognizable ad wrappers and media rather than generic "banner" classes.
  const adSelectors = [
    "ins.adsbygoogle", "[data-ad-client]", "[data-ad-slot]",
    "[id^='adfox_']", "[id^='yandex_rtb']", "#aclib", "[id^='aclib-']",
    ".ad-banner", ".ad-container", ".ad-wrapper", ".ad-slot",
    ".advertisement", ".advertising-block", ".popunder",
    ".in-page-ad", ".adsbygoogle", "iframe[src*='acscdn.com']",
    "iframe[src*='themoneytizer.com']",
  ];
  // File hosts and listing sites may put legitimate download controls
  // inside generic ad-styled containers. Keep explicit ad-network filtering
  // enabled, but avoid hiding entire download sections.
  if (["drive.online-fix.me", "hosters.online-fix.me",
      "fileditchfiles.st", "filekeeper.net", "pixeldrain.com",
      "gofile.io", "vikingfile.com",
      "game3rb.com", "www.game3rb.com",
      "fitgirl-repacks.site", "www.fitgirl-repacks.site"].includes(location.hostname)) {
    for (const name of [".ad-banner", ".ad-container", ".ad-wrapper",
        ".ad-slot", ".advertisement", ".advertising-block", ".in-page-ad"]) {
      const index = adSelectors.indexOf(name);
      if (index >= 0) adSelectors.splice(index, 1);
    }
  }
  const selector = adSelectors.join(",");
  const externalResources = "iframe,script,img,source,video,object,embed,link[rel='preload'],link[rel='stylesheet']";
  const linkSelector = "a[href]";
  const stylesheet = adSelectors.map(item => item + "{display:none!important;visibility:hidden!important;pointer-events:none!important;}").join("\n");

  function processElement(el) {
    if (!(el instanceof Element)) return;
    if (el.matches(selector) || (el.matches(externalResources) &&
        isAdvertisingUrl(el.getAttribute("src") || el.getAttribute("href") ||
          el.getAttribute("data-src") || el.getAttribute("data-url")))) {
      el.remove();
      return;
    }
    // Remove known ad embeds from asynchronously inserted content.
    for (const node of el.querySelectorAll(selector + "," + externalResources)) {
      if (node.matches(selector) ||
          isAdvertisingUrl(node.getAttribute("src") || node.getAttribute("href") ||
            node.getAttribute("data-src") || node.getAttribute("data-url"))) {
        node.remove();
      }
    }
  }

  function addStyles() {
    if (!document.head || document.getElementById("dusk-ad-block-style")) return;
    const style = document.createElement("style");
    style.id = "dusk-ad-block-style";
    style.textContent = stylesheet;
    document.head.append(style);
  }

  // Cancel clicks through invisible/sponsor links. Content links are untouched.
  document.addEventListener("click", (event) => {
    const el = event.target;
    if (!(el instanceof Element)) return;
    const anchor = el.closest(linkSelector);
    if (anchor && isAdvertisingUrl(anchor.getAttribute("href"))) {
      event.preventDefault();
      event.stopImmediatePropagation();
    }
  }, true);

  const init = () => {
    addStyles();
    processElement(document.documentElement);
    const observer = new MutationObserver(records => {
      for (const record of records) {
        if (record.type === "attributes") {
          processElement(record.target);
        } else {
          for (const node of record.addedNodes) processElement(node);
        }
      }
      addStyles();
    });
    observer.observe(document.documentElement, {
      childList: true, subtree: true, attributes: true,
      attributeFilter: ["src", "href", "data-src", "data-url", "class", "id"],
    });
  };
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init, { once: true });
  } else {
    init();
  }
})();
