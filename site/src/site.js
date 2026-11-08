// The Separatrix website's only script: the theme switch, the docs sidebar on small
// screens, copy buttons on code, and the download block, which asks GitHub for the latest
// release. No tracking, no third-party scripts. The page works without it: every download
// link then opens the releases page.
(function () {
  "use strict";

  var THEME_KEY = "separatrix-site-theme";
  var RELEASE_KEY = "separatrix-site-release";
  var RELEASE_TTL = 10 * 60 * 1000;
  var repo = document.documentElement.getAttribute("data-repo");
  var github = "https://github.com/" + repo;
  var data = {};
  try {
    data = JSON.parse(document.getElementById("site-data").textContent);
  } catch (e) {
    data = { assets: [], oses: {} };
  }

  function store(kind, key, value) {
    try {
      var s = kind === "local" ? window.localStorage : window.sessionStorage;
      if (value === undefined) return s.getItem(key);
      s.setItem(key, value);
    } catch (e) {
      return null;
    }
    return null;
  }

  // ---- Theme ---------------------------------------------------------------
  // The theme lives on <body data-theme>, not <html>, so components.css's projector-mode
  // root scale (html[data-theme="light"]) does not apply to the website.
  var media = window.matchMedia ? window.matchMedia("(prefers-color-scheme: light)") : null;
  function applyTheme(theme) {
    document.body.setAttribute("data-theme", theme);
    document.documentElement.setAttribute("data-site-theme", theme);
    var items = document.querySelectorAll("[data-theme-choice]");
    for (var i = 0; i < items.length; i++) {
      var on = items[i].getAttribute("data-theme-choice") === theme;
      items[i].setAttribute("aria-checked", on ? "true" : "false");
      items[i].tabIndex = on ? 0 : -1;
    }
  }
  function chooseTheme(theme, focus) {
    store("local", THEME_KEY, theme);
    applyTheme(theme);
    if (focus) {
      var el = document.querySelector('[data-theme-choice="' + theme + '"]');
      if (el) el.focus();
    }
  }
  applyTheme(document.body.getAttribute("data-theme") === "light" ? "light" : "dark");
  if (media && media.addEventListener) {
    media.addEventListener("change", function (e) {
      var saved = store("local", THEME_KEY);
      if (saved !== "dark" && saved !== "light") applyTheme(e.matches ? "light" : "dark");
    });
  }
  var group = document.querySelector(".site-theme");
  if (group) {
    group.addEventListener("click", function (e) {
      var b = e.target.closest("[data-theme-choice]");
      if (b) chooseTheme(b.getAttribute("data-theme-choice"), false);
    });
    // A radio group: the arrow keys move and select, Tab leaves the group.
    group.addEventListener("keydown", function (e) {
      var keys = { ArrowLeft: 1, ArrowUp: 1, ArrowRight: 1, ArrowDown: 1, Home: 1, End: 1 };
      if (!keys[e.key]) return;
      e.preventDefault();
      var current = document.body.getAttribute("data-theme");
      var next = current === "dark" ? "light" : "dark";
      if (e.key === "Home") next = "dark";
      if (e.key === "End") next = "light";
      chooseTheme(next, true);
    });
  }

  // ---- In-page links land below the sticky header ----------------------------
  // site.css pads the scroll by --site-header-h; the header wraps to two rows on phones.
  var siteHeader = document.querySelector(".site-header");
  function measureHeader() {
    if (siteHeader) document.documentElement.style.setProperty("--site-header-h", siteHeader.offsetHeight + "px");
  }
  measureHeader();
  if (siteHeader && window.ResizeObserver) new ResizeObserver(measureHeader).observe(siteHeader);
  else window.addEventListener("resize", measureHeader);

  // ---- Docs sidebar on small screens ----------------------------------------
  var side = document.querySelector("[data-side]");
  var toggle = document.querySelector("[data-side-toggle]");
  if (side && toggle) {
    side.classList.add("is-collapsible");
    toggle.addEventListener("click", function () {
      var open = toggle.getAttribute("aria-expanded") !== "true";
      toggle.setAttribute("aria-expanded", open ? "true" : "false");
      side.classList.toggle("is-open", open);
    });
  }

  // ---- Wide tables and code scroll inside their box; let the keyboard reach them ----
  function markScrollable() {
    var boxes = document.querySelectorAll(".site-table-wrap, .site-code pre");
    for (var i = 0; i < boxes.length; i++) {
      var el = boxes[i];
      if (el.scrollWidth > el.clientWidth + 1) {
        el.tabIndex = 0;
        if (!el.hasAttribute("aria-label")) el.setAttribute("aria-label", el.tagName === "PRE" ? "Code, scrolls sideways" : "Table, scrolls sideways");
        el.setAttribute("role", "region");
      } else if (el.getAttribute("role") === "region") {
        el.removeAttribute("tabindex");
        el.removeAttribute("role");
        el.removeAttribute("aria-label");
      }
    }
  }
  // ---- Copy buttons on code blocks -------------------------------------------
  var copyIcon =
    '<svg class="ds-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><rect x="5.5" y="5.5" width="8" height="8" rx="1.5"/><path d="M10.5 5.5v-2a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2"/></svg>';
  var checkIcon =
    '<svg class="ds-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><path d="M3 8.5 6.5 12 13 4.5"/></svg>';
  if (navigator.clipboard && window.isSecureContext) {
    var blocks = document.querySelectorAll(".site-code");
    for (var c = 0; c < blocks.length; c++) {
      (function (block) {
        var pre = block.querySelector("pre");
        var btn = document.createElement("button");
        btn.type = "button";
        btn.className = "ds-button ds-button--ghost ds-button--sm site-copy";
        btn.innerHTML = copyIcon + "<span>Copy</span>";
        btn.addEventListener("click", function () {
          navigator.clipboard.writeText(pre.textContent).then(function () {
            btn.innerHTML = checkIcon + "<span>Copied</span>";
            setTimeout(function () {
              btn.innerHTML = copyIcon + "<span>Copy</span>";
            }, 2000);
          });
        });
        block.appendChild(btn);
        block.classList.add("has-copy");
      })(blocks[c]);
    }
  }

  // After the copy buttons, which narrow the code boxes.
  markScrollable();
  window.addEventListener("resize", markScrollable);

  // ---- Download block ---------------------------------------------------------
  var blocksDl = document.querySelectorAll("[data-download]");
  if (!blocksDl.length) return;

  function detectOS() {
    var uad = navigator.userAgentData;
    var ua = navigator.userAgent || "";
    if ((uad && uad.mobile) || /Android|iPhone|iPad|iPod/i.test(ua)) return null;
    // iPadOS reports itself as a Mac; a Mac has no touch screen.
    if (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1) return null;
    var p = (uad && uad.platform) || navigator.platform || ua;
    if (/CrOS|Chrome OS/i.test(p + " " + ua)) return null;
    if (/mac/i.test(p)) return "macos";
    if (/win/i.test(p)) return "windows";
    if (/linux|x11/i.test(p)) return "linux";
    return null;
  }

  function size(bytes) {
    if (!(bytes > 0)) return "";
    if (bytes >= 1e6) return (bytes / 1e6).toFixed(1) + " MB";
    return Math.max(1, Math.round(bytes / 1e3)) + " KB";
  }

  function getRelease() {
    var cached = store("session", RELEASE_KEY);
    if (cached) {
      try {
        var c = JSON.parse(cached);
        if (Date.now() - c.t < RELEASE_TTL) return Promise.resolve(c.r);
      } catch (e) {}
    }
    var ctrl = window.AbortController ? new AbortController() : null;
    var timer = ctrl ? setTimeout(function () { ctrl.abort(); }, 10000) : null;
    return fetch("https://api.github.com/repos/" + repo + "/releases/latest", {
      headers: { Accept: "application/vnd.github+json" },
      signal: ctrl ? ctrl.signal : undefined,
    })
      .then(function (res) {
        if (res.status === 404) return { state: "none" };
        if (!res.ok) return { state: "error" };
        return res.json().then(function (rel) {
          var assets = (rel.assets || []).map(function (a) {
            return { name: a.name, size: a.size, url: a.browser_download_url };
          });
          return { state: "ok", tag: rel.tag_name, page: rel.html_url, assets: assets };
        });
      })
      .catch(function () {
        return { state: "error" };
      })
      .then(function (r) {
        if (timer) clearTimeout(timer);
        if (r.state !== "error") store("session", RELEASE_KEY, JSON.stringify({ t: Date.now(), r: r }));
        return r;
      });
  }

  function findAsset(release, version, suffix) {
    var exact = "Separatrix_" + version + "_" + suffix;
    var hit = null;
    for (var i = 0; i < release.assets.length; i++) {
      var a = release.assets[i];
      if (a.name === exact) return a;
      if (!hit && a.name.indexOf("Separatrix_") === 0 && a.name.slice(-(suffix.length + 1)) === "_" + suffix) hit = a;
    }
    return hit;
  }

  function assetFor(os) {
    for (var i = 0; i < data.assets.length; i++) {
      if (data.assets[i].os === os && data.assets[i].primary) return data.assets[i];
    }
    return null;
  }

  var detected = detectOS();

  function setupBlock(block) {
    var mode = block.getAttribute("data-download");
    var os = mode === "auto" ? detected : mode;
    var primary = block.querySelector("[data-primary]");
    var status = block.querySelector("[data-status]");
    var notice = block.querySelector("[data-notice]");
    var groups = block.querySelector("[data-groups]");
    var sums = block.querySelector("[data-sums]");
    var osName = os ? data.oses[os] : null;

    // Put the visitor's system first.
    if (mode === "auto" && os) {
      var mine = groups.querySelector('[data-os-group="' + os + '"]');
      if (mine) {
        groups.insertBefore(mine, groups.firstChild);
        mine.classList.add("is-yours");
      }
    }
    primary.textContent = osName ? "Download for " + osName : "Download Separatrix";
    status.textContent = "Looking up the latest release…";
    var first = os ? assetFor(os) : null;

    return getRelease().then(function (r) {
      if (r.state === "none") {
        primary.textContent = "Open the releases page";
        primary.href = github + "/releases";
        status.textContent = "";
        notice.innerHTML =
          '<div class="ds-diagnostic ds-diagnostic--block ds-diagnostic--info">' +
          '<svg class="ds-icon ds-diagnostic-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><circle cx="8" cy="8" r="6"/><path d="M8 7.2V11M8 5h.01"/></svg>' +
          '<span class="ds-diagnostic-body"><span class="ds-diagnostic-word">Note</span>' +
          '<span class="ds-diagnostic-msg">The first release is on its way. Watch the repository to hear when it is out.</span>' +
          '<span class="ds-diagnostic-actions"><a class="ds-button ds-button--sm" href="' + github + '">Watch on GitHub</a></span></span></div>';
        notice.hidden = false;
        var lists = block.querySelectorAll("[data-dl-list], .site-dl-needs");
        for (var i = 0; i < lists.length; i++) lists[i].hidden = true;
        sums.parentNode.hidden = true;
        block.setAttribute("data-state", "none");
        return;
      }
      if (r.state === "error") {
        // Every link already opens the releases page; say why there are no sizes.
        status.textContent = "Couldn't reach GitHub to list the files, so the links open the releases page.";
        block.setAttribute("data-state", "error");
        return;
      }
      var version = String(r.tag || "").replace(/^v/, "");
      var links = block.querySelectorAll("[data-suffix]");
      for (var j = 0; j < links.length; j++) {
        var suffix = links[j].getAttribute("data-suffix");
        var a = findAsset(r, version, suffix);
        var sizeEl = block.querySelector('[data-size-for="' + suffix + '"]');
        if (a) {
          links[j].href = a.url;
          links[j].setAttribute("data-asset", a.name);
          if (sizeEl) {
            sizeEl.textContent = size(a.size);
            sizeEl.className = "site-dl-size numeric-sm";
          }
        } else {
          links[j].href = r.page;
          if (sizeEl) {
            // Words, not a number: set them as a caption.
            sizeEl.textContent = "Not in this release";
            sizeEl.className = "site-dl-size caption";
          }
        }
      }
      var sumsAsset = null;
      for (var k = 0; k < r.assets.length; k++) if (r.assets[k].name === "SHA256SUMS.txt") sumsAsset = r.assets[k];
      sums.href = sumsAsset ? sumsAsset.url : r.page;
      var fa = first ? findAsset(r, version, first.suffix) : null;
      if (fa) {
        primary.href = fa.url;
        status.textContent = "Version " + version + " · " + first.label + " · " + size(fa.size);
      } else if (first) {
        // This release lacks the visitor's usual file: don't promise a download.
        primary.href = r.page;
        primary.textContent = "Open the release page";
        status.textContent = "Not in version " + version + " yet: " + first.label + " for " + osName + ".";
      } else {
        primary.href = r.page;
        primary.textContent = "Download Separatrix " + version;
        status.textContent = "For Windows, macOS and Linux. Choose a file below.";
      }
      block.setAttribute("data-state", "ok");
    });
  }

  for (var b = 0; b < blocksDl.length; b++) setupBlock(blocksDl[b]);
})();
