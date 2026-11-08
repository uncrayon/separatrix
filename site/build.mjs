// Builds the Separatrix website into site/dist from the repository's own sources:
//   docs/guide/*.md, docs/install/*.md   rendered to HTML (the docs stay the single source)
//   app/src/styles/ds/                   the vendored design system, copied as-is
//   scripts/release/assets.txt           the release file names the download block looks for
//   app/src/templates/templates.json     the gallery count and the examples' Strogatz sections
//   docs/guide/examples/*.sepx           the example projects, copied so they download
//
//   cd site && npm ci && node build.mjs
//
// Every URL inside the site is relative, so it works under any path.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import crypto from "node:crypto";
import MarkdownIt from "markdown-it";
import { REPO, BRANCH } from "./config.mjs";

const SITE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(SITE, "..");
const DIST = path.join(SITE, "dist");
const GITHUB = `https://github.com/${REPO}`;

const fail = (msg) => {
  console.error(`build: ${msg}`);
  process.exit(1);
};
const warn = (msg) => console.warn(`build: warning: ${msg}`);
const read = (rel) => fs.readFileSync(path.join(ROOT, rel), "utf8");
const exists = (rel) => fs.existsSync(path.join(ROOT, rel));
const posix = (p) => p.split(path.sep).join("/");
const esc = (s) =>
  String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

const VERSION = JSON.parse(read("app/package.json")).version;
if (!/^\d+\.\d+\.\d+$/.test(VERSION)) fail(`app/package.json version ${VERSION} is not MAJOR.MINOR.PATCH`);

const LICENSE_LINE = read("LICENSE").split("\n").find((l) => l.startsWith("Copyright"));
const LICENSE_NAME = read("LICENSE").split("\n")[0].trim(); // "MIT License"

const TEMPLATES = JSON.parse(read("app/src/templates/templates.json"));

// The download block's words for each release file. The file names themselves come from
// scripts/release/assets.txt; the build fails if the two disagree, so a new installer can't
// be released without the site offering it.
const OSES = {
  macos: { name: "macOS", needs: "Apple silicon or Intel" },
  windows: { name: "Windows", needs: "Windows 10 or 11, 64-bit" },
  linux: { name: "Linux", needs: "64-bit Intel or AMD, distributions from 2022 on" },
};
const FORMATS = {
  "macos_universal.dmg": {
    label: "Disk image (.dmg)",
    note: "Universal: one file for Apple silicon and Intel Macs.",
    primary: true,
  },
  "windows_x64-setup.exe": {
    label: "Installer (.exe)",
    note: "The standard installer. It downloads WebView2 if the PC lacks it.",
    primary: true,
  },
  "windows_x64_offline-setup.exe": {
    label: "Offline installer (.exe)",
    note: "For PCs without internet, or that can't download WebView2. It carries WebView2 inside, so it is much larger.",
  },
  "windows_x64.msi": {
    label: "MSI package (.msi)",
    note: "For IT deployment to every user of a PC. Needs an administrator.",
  },
  "linux_x86_64.AppImage": {
    label: "AppImage",
    note: "One file that runs without installing, on most distributions.",
    primary: true,
  },
  "linux_amd64.deb": {
    label: "Debian package (.deb)",
    note: "Ubuntu, Debian, Linux Mint and Pop!_OS.",
  },
  "linux_x86_64.rpm": {
    label: "RPM package (.rpm)",
    note: "Fedora, openSUSE and other RPM distributions.",
  },
};

function readAssets() {
  const lines = read("scripts/release/assets.txt").split("\n").filter((l) => l.trim());
  const head = lines[0].trim().split(/\s+/);
  const col = (name) => {
    const i = head.indexOf(name);
    if (i < 0) fail(`scripts/release/assets.txt has no '${name}' column`);
    return i;
  };
  const [cOs, cNeed, cSuffix] = [col("os"), col("need"), col("suffix")];
  const rows = lines.slice(1).map((l) => l.trim().split(/\s+/));
  const always = rows.filter((r) => r[cNeed] === "always");
  const assets = always.map((r) => ({ os: r[cOs], suffix: r[cSuffix] }));
  for (const a of assets) {
    if (!OSES[a.os]) fail(`assets.txt names os '${a.os}', which the site has no section for`);
    if (!FORMATS[a.suffix]) fail(`assets.txt releases '${a.suffix}', which FORMATS in site/build.mjs does not describe`);
  }
  for (const s of Object.keys(FORMATS)) {
    if (!assets.some((a) => a.suffix === s)) fail(`FORMATS in site/build.mjs offers '${s}', which assets.txt does not release`);
  }
  for (const os of Object.keys(OSES)) {
    const primaries = assets.filter((a) => a.os === os && FORMATS[a.suffix].primary);
    if (primaries.length !== 1) fail(`${os} needs exactly one primary format, has ${primaries.length}`);
  }
  return assets.map((a) => ({ ...a, ...FORMATS[a.suffix] }));
}
const ASSETS = readAssets();

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

// Pages: the guide index and every guide page it links (and they link), plus the install guides.
const PAGE_SOURCES = new Map(); // repo path -> output path (relative to dist)
const outFor = (rel) => {
  const m = rel.match(/^docs\/(guide|install)\/([a-z0-9-]+|README)\.md$/);
  if (!m) return null;
  return `${m[1]}/${m[2] === "README" ? "index" : m[2]}.html`;
};

const md = new MarkdownIt({ html: false, linkify: false, typographer: false });

// GitHub's heading anchors, so links such as linux.md#start-separatrix-for-the-first-time work.
function slug(text) {
  return text
    .toLowerCase()
    .trim()
    .replace(/[^\p{L}\p{M}\p{N}\p{Pc} -]/gu, "")
    .replace(/ /g, "-");
}
const inlineText = (tok) => (tok.children || []).map((c) => (c.type === "text" || c.type === "code_inline" ? c.content : "")).join("");

// The install guides carry notes for the owner ("> **Screenshot to add on a real Mac:** …").
// They are reminders for the repository, not for readers, so the site leaves them out.
md.core.ruler.after("block", "drop_screenshot_notes", (state) => {
  const t = state.tokens;
  for (let i = 0; i < t.length; i++) {
    if (t[i].type !== "blockquote_open") continue;
    let depth = 0;
    let j = i;
    for (; j < t.length; j++) {
      if (t[j].type === "blockquote_open") depth++;
      if (t[j].type === "blockquote_close" && --depth === 0) break;
    }
    const firstInline = t.slice(i, j).find((x) => x.type === "inline");
    if (firstInline && /^\*\*Screenshot to add\b/.test(firstInline.content)) {
      t.splice(i, j - i + 1);
      i--;
    }
  }
});

const TYPE_FOR_HEADING = { h1: "display", h2: "title", h3: "heading", h4: "body-strong", h5: "body-strong", h6: "body-strong" };

md.renderer.rules.heading_open = (tokens, idx, opts, env, self) => {
  const tok = tokens[idx];
  const text = inlineText(tokens[idx + 1]);
  let id = slug(text);
  const n = env.ids.get(id) || 0;
  env.ids.set(id, n + 1);
  if (n) id = `${id}-${n}`;
  env.anchors.add(id);
  if (tok.tag === "h1" && !env.title) env.title = text;
  tok.attrSet("id", id);
  tok.attrJoin("class", TYPE_FOR_HEADING[tok.tag]);
  return self.renderToken(tokens, idx, opts);
};
for (const [rule, cls] of [
  ["paragraph_open", "body"],
  ["bullet_list_open", "body"],
  ["ordered_list_open", "body"],
]) {
  md.renderer.rules[rule] = (tokens, idx, opts, env, self) => {
    tokens[idx].attrJoin("class", cls);
    return self.renderToken(tokens, idx, opts);
  };
}
md.renderer.rules.table_open = () => '<div class="site-table-wrap"><table class="site-table body">\n';
md.renderer.rules.table_close = () => "</table></div>\n";
md.renderer.rules.th_open = (tokens, idx, opts, env, self) => {
  tokens[idx].attrJoin("class", "label");
  tokens[idx].attrSet("scope", "col");
  return self.renderToken(tokens, idx, opts);
};
// Strogatz writes x_{n+1} and x_n; the docs spell them in plain text. Set the index as a
// subscript in running text (code spans are left alone).
md.renderer.rules.text = (tokens, idx) =>
  esc(tokens[idx].content)
    .replace(/(?<![\p{L}\p{N}_])([A-Za-z])_\{([^{}]+)\}/gu, '<span class="site-math">$1<sub>$2</sub></span>')
    .replace(/(?<![\p{L}\p{N}_])([A-Za-z])_([A-Za-z0-9])(?![\p{L}\p{N}_])/gu, '<span class="site-math">$1<sub>$2</sub></span>');
md.renderer.rules.code_inline = (tokens, idx) => `<code class="code">${esc(tokens[idx].content)}</code>`;
md.renderer.rules.fence = (tokens, idx) => {
  const tok = tokens[idx];
  const lang = tok.info.trim().split(/\s+/)[0];
  const attr = lang ? ` data-lang="${esc(lang)}"` : "";
  return `<div class="site-code"><pre class="code"${attr}><code>${esc(tok.content.replace(/\n$/, ""))}</code></pre></div>\n`;
};
md.renderer.rules.code_block = md.renderer.rules.fence;

md.renderer.rules.link_open = (tokens, idx, opts, env, self) => {
  const tok = tokens[idx];
  const href = rewriteHref(tok.attrGet("href"), env);
  tok.attrSet("href", href.url);
  if (href.download) tok.attrSet("download", "");
  return self.renderToken(tokens, idx, opts);
};
md.renderer.rules.image = (tokens, idx, opts, env) => {
  const tok = tokens[idx];
  const src = tok.attrGet("src");
  const alt = inlineText(tok) || tok.content;
  const ref = rewriteHref(src, env, { asset: true });
  const dims = ref.dims ? ` width="${ref.dims.width}" height="${ref.dims.height}"` : "";
  return `<img src="${esc(ref.url)}" alt="${esc(alt)}"${dims} loading="lazy" decoding="async">`;
};

// A link whose text is the target page's .md file name ("[cobweb.md](cobweb.md)", as in the
// guide index's table) reads as the target page's title on the site.
const TITLES = new Map(); // repo path -> the page's first h1
md.core.ruler.push("page_link_titles", (state) => {
  const env = state.env;
  if (!env || !env.src) return;
  for (const block of state.tokens) {
    const kids = block.children || [];
    for (let i = 0; i + 2 < kids.length; i++) {
      const [open, text, close] = [kids[i], kids[i + 1], kids[i + 2]];
      if (open.type !== "link_open" || close.type !== "link_close") continue;
      if (text.type !== "text" && text.type !== "code_inline") continue;
      const href = open.attrGet("href") || "";
      if (/^[a-z][a-z0-9+.-]*:/i.test(href) || !/\.md$/.test(text.content)) continue;
      const dir = path.posix.dirname(env.src);
      const target = posix(path.posix.normalize(path.posix.join(dir, href.split("#")[0])));
      const named = posix(path.posix.normalize(path.posix.join(dir, text.content)));
      if (target !== named || !PAGE_SOURCES.has(target) || !TITLES.get(target)) continue;
      const t = new state.Token("text", "", 0);
      t.content = navTitle(TITLES.get(target));
      kids[i + 1] = t;
    }
  }
});

// Files that must land in dist: repo path -> dist path.
const COPIES = new Map();
// Links to check once every page is rendered: { from, page, anchor }.
const ANCHOR_LINKS = [];

function pngSize(abs) {
  const b = fs.readFileSync(abs);
  if (b.toString("ascii", 1, 4) !== "PNG") return null;
  return { width: b.readUInt32BE(16), height: b.readUInt32BE(20) };
}

const relFrom = (fromOut, toOut) => {
  const r = path.posix.relative(path.posix.dirname(fromOut), toOut);
  return r === "" ? path.posix.basename(toOut) : r;
};

// Map a link in a docs page to a URL that works in the site.
function rewriteHref(href, env, { asset = false } = {}) {
  if (!href) fail(`${env.src}: empty link`);
  if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("//")) return { url: href };
  if (href.startsWith("#")) {
    ANCHOR_LINKS.push({ from: env.src, page: env.out, anchor: href.slice(1) });
    return { url: href };
  }
  const [p, hash = ""] = href.split("#");
  const repoRel = posix(path.posix.normalize(path.posix.join(path.posix.dirname(env.src), p)));
  if (repoRel.startsWith("..")) fail(`${env.src}: link ${href} leaves the repository`);
  const frag = hash ? `#${hash}` : "";

  if (PAGE_SOURCES.has(repoRel)) {
    const target = PAGE_SOURCES.get(repoRel);
    if (hash) ANCHOR_LINKS.push({ from: env.src, page: target, anchor: hash });
    return { url: relFrom(env.out, target) + frag };
  }
  if (!exists(repoRel)) fail(`${env.src}: link ${href} points at ${repoRel}, which does not exist`);
  const abs = path.join(ROOT, repoRel);
  if (fs.statSync(abs).isDirectory()) {
    if (repoRel === "docs/guide/examples") return { url: relFrom(env.out, "index.html") + "#examples" };
    return { url: `${GITHUB}/tree/${BRANCH}/${repoRel}` };
  }
  const isImage = /\.(png|jpe?g|gif|svg|webp)$/i.test(repoRel);
  const isExample = /^docs\/guide\/examples\/[^/]+\.sepx$/.test(repoRel);
  if ((asset || isImage || isExample) && repoRel.startsWith("docs/")) {
    const out = repoRel.slice("docs/".length);
    COPIES.set(repoRel, out);
    return {
      url: relFrom(env.out, out) + frag,
      dims: /\.png$/i.test(repoRel) ? pngSize(abs) : null,
      download: isExample,
    };
  }
  if (asset) fail(`${env.src}: image ${href} is outside docs/`);
  return { url: `${GITHUB}/blob/${BRANCH}/${repoRel}${frag}` };
}

function collectPages() {
  const queue = ["docs/guide/README.md", "docs/install/macos.md", "docs/install/windows.md", "docs/install/linux.md"];
  for (const q of queue) if (!exists(q)) fail(`${q} is missing`);
  while (queue.length) {
    const src = queue.shift();
    if (PAGE_SOURCES.has(src)) continue;
    PAGE_SOURCES.set(src, outFor(src));
    if (!src.startsWith("docs/guide/")) continue;
    for (const tok of md.parse(read(src), {}).flatMap((t) => t.children || [])) {
      if (tok.type !== "link_open") continue;
      const href = tok.attrGet("href").split("#")[0];
      if (!href.endsWith(".md") || /^[a-z]+:/i.test(href)) continue;
      const rel = posix(path.posix.normalize(path.posix.join(path.posix.dirname(src), href)));
      if (outFor(rel) && rel.startsWith("docs/guide/") && exists(rel)) queue.push(rel);
    }
  }
  for (const src of PAGE_SOURCES.keys()) {
    const tokens = md.parse(read(src), {});
    const h1 = tokens.findIndex((t) => t.type === "heading_open" && t.tag === "h1");
    if (h1 >= 0) TITLES.set(src, plain(tokens[h1 + 1]));
  }
}

function renderPage(src) {
  const env = { src, out: PAGE_SOURCES.get(src), ids: new Map(), anchors: new Set(), title: "" };
  const text = read(src);
  const tokens = md.parse(text, env);
  const html = md.renderer.render(tokens, md.options, env);
  const firstPara = tokens.findIndex((t) => t.type === "paragraph_open");
  const description = firstPara >= 0 ? plain(tokens[firstPara + 1]) : "";
  return { ...env, html, tokens, description };
}
const plain = (inline) => inlineText(inline).replace(/\s+/g, " ").trim();
const firstSentence = (s) => s.split(/(?<=\.)\s+(?=[A-Z])/)[0];

// ---------------------------------------------------------------------------
// Navigation from the guide index: its h2 sections and the pages each links, in order.
// ---------------------------------------------------------------------------
function guideNav(pages) {
  const tokens = md.parse(read("docs/guide/README.md"), {});
  const groups = [];
  let group = null;
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (t.type === "heading_open" && t.tag === "h2") {
      group = { title: plain(tokens[i + 1]), pages: [] };
      groups.push(group);
    }
    if (!group || t.type !== "inline") continue;
    for (const c of t.children) {
      if (c.type !== "link_open") continue;
      const rel = posix(path.posix.normalize(path.posix.join("docs/guide", c.attrGet("href").split("#")[0])));
      if (rel.startsWith("docs/guide/") && PAGE_SOURCES.has(rel) && !group.pages.includes(rel)) group.pages.push(rel);
    }
  }
  const listed = new Set(groups.flatMap((g) => g.pages));
  const extra = [...PAGE_SOURCES.keys()].filter((s) => s.startsWith("docs/guide/") && !s.endsWith("README.md") && !listed.has(s));
  const nav = [
    { title: "Install", pages: ["install/index.html", ...["macos", "windows", "linux"].map((o) => `docs/install/${o}.md`)] },
    { title: "User guide", pages: ["docs/guide/README.md"] },
    ...groups.filter((g) => g.pages.length),
  ];
  if (extra.length) nav.push({ title: "More", pages: extra });
  return nav.map((g) => ({
    title: g.title,
    items: g.pages.map((p) =>
      p === "install/index.html"
        ? { out: p, title: "Choose your system" }
        : { out: PAGE_SOURCES.get(p), title: p.endsWith("README.md") ? "Overview" : navTitle(pages.get(p).title) },
    ),
  }));
}
const navTitle = (t) => t.replace(/^Install Separatrix on (a Mac|Windows|Linux)$/, (_, os) => (os === "a Mac" ? "macOS" : os));

// ---------------------------------------------------------------------------
// HTML pieces
// ---------------------------------------------------------------------------
const ICON = {
  logo: (cls = "") =>
    `<svg class="ds-icon site-logo ${cls}" viewBox="0 0 20 20" fill="none" stroke="currentColor" aria-hidden="true" focusable="false"><circle cx="10" cy="10" r="9" stroke-width="1.4" opacity="0.35"/><path d="M10 10c0-1.2 1.6-1.6 2.4-.6 1.2 1.5-.2 3.8-2.4 3.8-2.8 0-4.3-3.1-2.8-5.6 1.9-3.2 6.9-2.9 8.4.6" stroke-width="1.6" stroke-linecap="round"/><circle cx="10" cy="10" r="1.3" fill="currentColor" stroke="none"/></svg>`,
  stroke: (d) =>
    `<svg class="ds-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${d}</svg>`,
};
const ICONS = {
  info: ICON.stroke('<circle cx="8" cy="8" r="6"/><path d="M8 7.2V11M8 5h.01"/>'),
  warning: ICON.stroke('<path d="M8 2.2 14.2 13H1.8L8 2.2Z"/><path d="M8 6.5v3M8 11.2h.01"/>'),
  copy: ICON.stroke('<rect x="5.5" y="5.5" width="8" height="8" rx="1.5"/><path d="M10.5 5.5v-2a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2"/>'),
  check: ICON.stroke('<path d="M3 8.5 6.5 12 13 4.5"/>'),
  chevron: ICON.stroke('<path d="M4.5 6.5 8 10l3.5-3.5"/>'),
};

function header(root, current) {
  const link = (href, label, key) =>
    `<a class="site-nav-link body-strong" href="${href}"${current === key ? ' aria-current="page"' : ""}>${label}</a>`;
  return `<header class="site-header">
  <div class="site-header-inner site-container">
    <a class="site-brand" href="${root}index.html">${ICON.logo()}<span class="title">Separatrix</span></a>
    <nav class="site-nav" aria-label="Main">
      ${link(`${root}index.html#download`, "Download", "download")}
      ${link(`${root}install/index.html`, "Install", "install")}
      ${link(`${root}guide/index.html`, "Guide", "guide")}
      ${link(GITHUB, "GitHub", "github")}
    </nav>
    <div class="ds-segmented ds-segmented--sm site-theme" role="radiogroup" aria-label="Theme">
      <button type="button" class="ds-segmented-item" role="radio" aria-checked="true" data-theme-choice="dark">Dark</button>
      <button type="button" class="ds-segmented-item" role="radio" aria-checked="false" data-theme-choice="light" tabindex="-1">Light</button>
    </div>
  </div>
</header>`;
}

function footer(root) {
  return `<footer class="site-footer">
  <div class="site-container site-footer-inner caption">
    <p class="site-footer-brand">${ICON.logo()}<span>Separatrix <span class="numeric-sm">${VERSION}</span></span></p>
    <p>${esc(LICENSE_NAME)}. ${esc(LICENSE_LINE.replace(/^Copyright\s+\(c\)/i, "©"))}.</p>
    <p><a href="${GITHUB}">Source on GitHub</a> · <a href="${GITHUB}/blob/${BRANCH}/CHANGELOG.md">Changelog</a> · <a href="${GITHUB}/blob/${BRANCH}/LICENSE">License</a></p>
  </div>
</footer>`;
}

function layout({ root, title, description, current, main }) {
  const fullTitle = title === "Separatrix" ? title : `${title} · Separatrix`;
  return `<!doctype html>
<html lang="en" data-repo="${esc(REPO)}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(fullTitle)}</title>
<meta name="description" content="${esc(description)}">
<meta name="color-scheme" content="dark light">
<link rel="icon" href="${root}assets/logo.svg" type="image/svg+xml">
<link rel="stylesheet" href="${root}ds/tokens.css">
<link rel="stylesheet" href="${root}ds/components.css">
<link rel="stylesheet" href="${root}assets/site.css">
<script type="application/json" id="site-data">${JSON.stringify({ assets: ASSETS.map(({ os, suffix, label, primary }) => ({ os, suffix, label, primary: !!primary })), oses: Object.fromEntries(Object.entries(OSES).map(([k, v]) => [k, v.name])) }).replace(/</g, "\\u003c")}</script>
<script src="${root}assets/site.js" defer></script>
</head>
<body class="ds-root">
<script>${THEME_BOOT}</script>
<a class="ds-button site-skip" href="#main">Skip to content</a>
${header(root, current)}
${main}
${footer(root)}
</body>
</html>
`;
}

// Runs before the first paint, so the page never flashes the wrong theme. The theme goes on
// <body>, not <html>: components.css scales the root to 115% for html[data-theme="light"]
// (projector mode), which the website does not want. html[data-site-theme] sets color-scheme.
const THEME_BOOT = `(function(){var t;try{t=localStorage.getItem("separatrix-site-theme")}catch(e){}if(t!=="dark"&&t!=="light"){t=window.matchMedia&&matchMedia("(prefers-color-scheme: light)").matches?"light":"dark"}document.body.setAttribute("data-theme",t);document.documentElement.setAttribute("data-site-theme",t)})();`;

// The download block. Its static links all open the releases page, so it works before (or
// without) the script; src/site.js then fills in the real files of the latest release.
function downloadBlock(root, { os = "auto", heading = "Download", headingLevel = 2, id = "download" } = {}) {
  const releases = `${GITHUB}/releases/latest`;
  const groups = (os === "auto" ? Object.keys(OSES) : [os])
    .map((key) => {
      const items = ASSETS.filter((a) => a.os === key)
        .sort((a, b) => Object.keys(FORMATS).indexOf(a.suffix) - Object.keys(FORMATS).indexOf(b.suffix))
        .map(
          (a) => `<li class="site-dl-item">
            <a class="site-dl-file body-strong" href="${releases}" data-suffix="${esc(a.suffix)}">${esc(a.label)}</a>
            <span class="site-dl-size numeric-sm" data-size-for="${esc(a.suffix)}"></span>
            <span class="site-dl-note caption">${esc(a.note)}</span>
          </li>`,
        )
        .join("\n");
      const guide = os === "auto" ? `<a class="site-dl-guide label" href="${root}install/${key}.html">Install guide for ${OSES[key].name}</a>` : "";
      // A block for one system is already titled with its name.
      const name = os === "auto" ? `<h${headingLevel + 1} class="heading">${OSES[key].name}</h${headingLevel + 1}>` : "";
      return `<div class="site-dl-os" data-os-group="${key}">
        ${name}
        <p class="caption site-dl-needs">${esc(OSES[key].needs)}</p>
        <ul class="site-dl-list" data-dl-list>${items}</ul>
        ${guide}
      </div>`;
    })
    .join("\n");
  const primaryLabel = os === "auto" ? "Download Separatrix" : `Download for ${OSES[os].name}`;
  return `<section class="site-dl" id="${id}" aria-labelledby="${id}-title" data-download="${os}">
  <div class="site-dl-head">
    <h${headingLevel} class="title" id="${id}-title">${esc(heading)}</h${headingLevel}>
    <p class="caption site-dl-free">Free and open source, ${esc(LICENSE_NAME.replace(/ License$/, ""))} license</p>
  </div>
  <div class="site-dl-primary">
    <a class="ds-button ds-button--primary" href="${releases}" data-primary>${primaryLabel}</a>
    <p class="caption site-dl-status" data-status aria-live="polite">Opens the releases page on GitHub.</p>
  </div>
  <div class="site-dl-notice" data-notice hidden></div>
  <div class="site-dl-groups${os === "auto" ? "" : " site-dl-groups--one"}" data-groups>
    ${groups}
  </div>
  <p class="caption site-dl-foot"><a href="${releases}" data-sums>SHA256SUMS.txt</a> lists the SHA-256 checksum of every file. · <a href="${GITHUB}/releases">All releases</a></p>
</section>`;
}

const kb = (bytes) => `${(bytes / 1024).toFixed(1)} KB`;

// [2, 3, 4, 5, 12] -> "2 to 5 and 12": runs of consecutive chapters, so a gap shows.
function chapterList(nums) {
  const runs = [];
  for (const n of nums) {
    const last = runs[runs.length - 1];
    if (last && n === last[1] + 1) last[1] = n;
    else runs.push([n, n]);
  }
  const parts = runs.map(([a, b]) => (a === b ? `${a}` : b === a + 1 ? `${a} and ${b}` : `${a} to ${b}`));
  const words = parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}` : parts[0];
  return `${runs.length === 1 && runs[0][0] === runs[0][1] ? "chapter" : "chapters"} ${words}`;
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------
function docsPage({ out, nav, title, description, content, current, sourceRel }) {
  const root = "../".repeat(out.split("/").length - 1);
  const flat = nav.flatMap((g) => g.items);
  const at = flat.findIndex((i) => i.out === out);
  const prev = at > 0 ? flat[at - 1] : null;
  const next = at >= 0 && at < flat.length - 1 ? flat[at + 1] : null;
  const sidebar = nav
    .map(
      (g, gi) => `<div class="site-side-group">
      <p class="label site-side-title" id="side-${gi}">${esc(g.title)}</p>
      <ul class="site-side-list" aria-labelledby="side-${gi}">
        ${g.items
          .map(
            (i) =>
              `<li><a class="site-side-link body" href="${relFrom(out, i.out)}"${i.out === out ? ' aria-current="page"' : ""}>${esc(i.title)}</a></li>`,
          )
          .join("\n")}
      </ul>
    </div>`,
    )
    .join("\n");
  const pager =
    prev || next
      ? `<nav class="site-pager" aria-label="Previous and next page">
      ${prev ? `<a class="site-pager-link" href="${relFrom(out, prev.out)}" rel="prev"><span class="caption">Previous</span><span class="body-strong">${esc(prev.title)}</span></a>` : "<span></span>"}
      ${next ? `<a class="site-pager-link site-pager-link--next" href="${relFrom(out, next.out)}" rel="next"><span class="caption">Next</span><span class="body-strong">${esc(next.title)}</span></a>` : "<span></span>"}
    </nav>`
      : "";
  const source = sourceRel
    ? `<p class="caption site-source">This page is <a href="${GITHUB}/blob/${BRANCH}/${sourceRel}">${esc(sourceRel)}</a> in the repository.</p>`
    : "";
  const main = `<div class="site-container site-docs">
  <nav class="site-side" aria-label="Guide pages" data-side>
    <button type="button" class="ds-button ds-button--ghost site-side-toggle" aria-expanded="false" aria-controls="site-side-body" data-side-toggle>${ICONS.chevron}<span>Pages</span></button>
    <div class="site-side-body" id="site-side-body">${sidebar}</div>
  </nav>
  <main id="main" class="site-main" tabindex="-1">
    <article class="site-prose">
${content}
    </article>
    ${pager}
    ${source}
  </main>
</div>`;
  write(out, layout({ root, title, description, current, main }));
}

function write(out, html) {
  const abs = path.join(DIST, out);
  fs.mkdirSync(path.dirname(abs), { recursive: true });
  fs.writeFileSync(abs, html);
}
function copy(fromRel, toOut) {
  const abs = path.join(DIST, toOut);
  fs.mkdirSync(path.dirname(abs), { recursive: true });
  fs.copyFileSync(path.join(ROOT, fromRel), abs);
}

function homePage(pages) {
  const root = "";
  const manifest = JSON.parse(read("site/media/manifest.json"));
  // A committed web copy is used only while it still matches its source screenshot.
  const sha256 = (rel) => crypto.createHash("sha256").update(fs.readFileSync(path.join(ROOT, rel))).digest("hex");
  return Promise.all(
    readmeViews().map(async (v) => {
      const m = manifest[v.key];
      let img;
      if (m && exists(`site/media/${m.file}`) && sha256(m.source) === m.sha256) {
        COPIES.set(`site/media/${m.file}`, `media/${m.file}`);
        img = { src: `media/${m.file}`, width: m.width, height: m.height };
      } else {
        warn(`site/media/views/${v.key}.webp is missing or older than docs/media/${v.key}.png; run bash site/media.sh. Using the full-size PNG.`);
        COPIES.set(`docs/media/${v.key}.png`, `media/${v.key}.png`);
        img = { src: `media/${v.key}.png`, ...pngSize(path.join(ROOT, `docs/media/${v.key}.png`)) };
      }
      return { ...v, img };
    }),
  ).then((views) => {
    const video = manifest.video;
    if (!video || !exists(`site/media/${video.file}`) || !exists(`site/media/${video.poster}`)) fail("site/media is missing the video or its poster");
    COPIES.set(`site/media/${video.file}`, `media/${video.file}`);
    COPIES.set(`site/media/${video.poster}`, `media/${video.poster}`);

    // What the View control offers, in the guide index's own words (the first paragraph of
    // its "The views" section).
    const guideTokens = pages.get("docs/guide/README.md").tokens;
    const viewsAt = guideTokens.findIndex((t, i) => t.type === "heading_open" && t.tag === "h2" && plain(guideTokens[i + 1]) === "The views");
    const viewsPara = viewsAt >= 0 ? guideTokens.findIndex((t, i) => i > viewsAt && t.type === "paragraph_open") : -1;
    if (viewsPara < 0) fail("docs/guide/README.md has no paragraph under '## The views'");
    const viewsLede = md.renderInline(guideTokens[viewsPara + 1].content, { src: "docs/guide/README.md", out: "index.html", ids: new Map(), anchors: new Set() });

    const chapters = [...new Set(TEMPLATES.map((t) => Number(String(t.chapter).replace(/\D/g, ""))))].sort((a, b) => a - b);
    const viewCards = views
      .map(
        (v) => `<li class="site-card">
        <a class="site-card-shot" href="${v.page}" tabindex="-1" aria-hidden="true"><img src="${v.img.src}" alt="" width="${v.img.width}" height="${v.img.height}" loading="lazy" decoding="async"></a>
        <div class="site-card-body">
          <h3 class="heading"><a class="site-card-title" href="${v.page}">${esc(v.title)}</a></h3>
          <p class="body site-card-text">${v.summary}</p>
          <p class="caption site-card-example">Example: ${esc(v.template.title)}, Strogatz §${esc(v.template.section)}</p>
          <p class="label site-card-links"><a href="${v.page}">Read the guide</a><a href="${v.example}" download>Download ${esc(path.posix.basename(v.example))}</a></p>
        </div>
      </li>`,
      )
      .join("\n");
    const exampleRows = views
      .map(
        (v) => `<tr>
          <td><a class="code" href="${v.example}" download>${esc(path.posix.basename(v.example))}</a></td>
          <td>${esc(v.template.title)}</td>
          <td class="numeric">§${esc(v.template.section)}</td>
          <td><a href="${v.page}">${esc(v.title)}</a></td>
          <td class="numeric site-num">${kb(fs.statSync(path.join(ROOT, v.exampleSrc)).size)}</td>
        </tr>`,
      )
      .join("\n");

    const main = `<main id="main" tabindex="-1">
  <section class="site-hero site-container" aria-labelledby="hero-title">
    <div class="site-hero-text">
      <h1 class="display site-hero-name" id="hero-title">${ICON.logo("site-hero-logo")}Separatrix</h1>
      <p class="body site-hero-lede">A free desktop app for exploring dynamical systems: type a system the way you write it on the board, and watch its phase portrait change as you drag a parameter.</p>
      <p class="body site-hero-who">For students and professors of mathematics and physics. Windows, macOS and Linux. Works offline.</p>
    </div>
    <div class="site-hero-dl">${downloadBlock(root)}</div>
    <figure class="site-video">
      <video controls muted playsinline preload="none" poster="media/${video.poster}" width="${video.width}" height="${video.height}" aria-describedby="video-caption">
        <source src="media/${video.file}" type="video/mp4">
      </video>
      <figcaption class="caption" id="video-caption">A tour in 87 seconds, with no sound: the phase plane, a parameter drag across a saddle-node bifurcation, the Lorenz equations in 3D, Lyapunov exponents, the logistic map, LaTeX export and projector mode. <a href="media/${video.file}" download>Download the video</a> (MP4, ${(fs.statSync(path.join(ROOT, "site/media", video.file)).size / 1e6).toFixed(1)} MB).</figcaption>
    </figure>
  </section>

  <section class="site-section site-container" id="views" aria-labelledby="views-title">
    <h2 class="title" id="views-title">What you can do</h2>
    <p class="body site-section-lede">${viewsLede} Each guide page below links an example project that opens in that view.</p>
    <ul class="site-cards">${viewCards}</ul>
  </section>

  <section class="site-section site-container" id="examples" aria-labelledby="examples-title">
    <h2 class="title" id="examples-title">Examples</h2>
    <p class="body site-section-lede">Download a project and open it with <strong>Open…</strong>, or drop it on the Separatrix window. Each file is a few kilobytes: it holds the equations, the start points and the view, and Separatrix recomputes the rest.</p>
    <div class="site-table-wrap"><table class="site-table body">
      <thead><tr><th scope="col" class="label">Project</th><th scope="col" class="label">System</th><th scope="col" class="label">Strogatz</th><th scope="col" class="label">Opens in</th><th scope="col" class="label site-num">Size</th></tr></thead>
      <tbody>${exampleRows}</tbody>
    </table></div>
    <p class="body site-gallery-note">The app also opens on a gallery of <span class="numeric">${TEMPLATES.length}</span> examples from ${chapterList(chapters)} of Strogatz's <cite>Nonlinear Dynamics and Chaos</cite> (2nd edition), grouped by chapter. Click a card to start from that system.</p>
  </section>

  <section class="site-section site-container" id="why" aria-labelledby="why-title">
    <h2 class="title" id="why-title">Why Separatrix</h2>
    <ul class="site-why">
      <li><h3 class="heading">Works offline</h3><p class="body">Separatrix needs no internet connection after you install it. The solvers and the plots run on your computer.</p></li>
      <li><h3 class="heading">LaTeX first</h3><p class="body">Copy a pgfplots figure, the system in an <code class="code">align</code> environment, or a booktabs table of fixed points, Lyapunov exponents or bifurcations, ready for your notes. SVG, PNG and CSV too.</p></li>
      <li><h3 class="heading">Strogatz's notation</h3><p class="body">Type <code class="code">\\dot{x}</code> and <code class="code">x_{n+1}</code> as on the board in LaTeX, or <code class="code">x'</code> in plain text. Filled dots are stable fixed points and open circles unstable ones, named in his words: stable spiral, saddle, center.</p></li>
      <li><h3 class="heading">Projector mode</h3><p class="body">A light theme with type 15% larger and plot lines 1.75 times as thick, so a plot reads from the back of a lecture hall.</p></li>
      <li><h3 class="heading">Small project files</h3><p class="body">A <code class="code">.sepx</code> file holds what you typed and chose, not computed data, so it is a few kilobytes and travels by email or a course platform.</p></li>
      <li><h3 class="heading">Checked against the book</h3><p class="body">Three gates check the numbers on every change: Strogatz's chapter 2 to 8 results, published Lyapunov exponents and closed-form bifurcation points. For Lorenz, <span class="site-math">λ<sub>1</sub></span> = <span class="numeric">0.9051 ± 0.0013</span> against Sprott's <span class="numeric">0.9056</span>.</p></li>
    </ul>
    <p class="body site-section-more"><a href="guide/getting-started.html">Start the ten-minute tutorial</a> or <a href="guide/index.html">read the user guide</a>.</p>
  </section>
</main>`;
    write(
      "index.html",
      layout({
        root,
        title: "Separatrix",
        description:
          "Separatrix is a free, offline desktop app for exploring dynamical systems, for students and professors of mathematics and physics. Windows, macOS and Linux.",
        current: "",
        main,
      }),
    );
  });

  // The eight views, in the order of the guide index's table, with each guide page's own
  // title, first sentence, and example project.
  function readmeViews() {
    const rows = read("docs/guide/README.md")
      .split("\n")
      .filter((l) => /^\|.*\]\(examples\/[^)]+\.sepx\)/.test(l));
    if (rows.length !== 8) fail(`docs/guide/README.md's views table has ${rows.length} rows with an example, expected 8`);
    return rows.map((row) => {
      const pageMd = row.match(/\]\(([a-z0-9-]+\.md)\)/)[1];
      const exampleMd = row.match(/\]\((examples\/[a-z0-9-]+\.sepx)\)/)[1];
      const key = pageMd.replace(/\.md$/, "");
      const src = `docs/guide/${pageMd}`;
      const page = pages.get(src);
      if (!page) fail(`${src} is not a page`);
      const exampleSrc = `docs/guide/${exampleMd}`;
      COPIES.set(exampleSrc, `guide/${exampleMd}`);
      const project = JSON.parse(read(exampleSrc));
      const template = TEMPLATES.find((t) => t.id === project.system?.template);
      if (!template || !template.section) fail(`${exampleSrc} names template ${project.system?.template}, which has no section in templates.json`);
      const para = page.tokens.findIndex((t) => t.type === "paragraph_open");
      const sentence = firstSentence(page.tokens[para + 1].content);
      return {
        key,
        title: page.title,
        page: PAGE_SOURCES.get(src),
        example: `guide/${exampleMd}`,
        exampleSrc,
        template,
        summary: md.renderInline(sentence, { src, out: "index.html", ids: new Map(), anchors: new Set() }),
      };
    });
  }
}

function installIndex(nav) {
  const content = `<h1 class="display" id="install-separatrix">Install Separatrix</h1>
<p class="body">Separatrix runs on Windows, macOS and Linux, and needs no internet connection after you install it. Download the file for your computer, then follow its guide: it takes you through the checksum, the first launch and your first project.</p>
${downloadBlock("../", { heading: "Download", id: "download" })}`;
  docsPage({
    out: "install/index.html",
    nav,
    title: "Install",
    description: "Download Separatrix for Windows, macOS or Linux, and follow the install guide for your computer.",
    content,
    current: "install",
  });
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
async function main() {
  fs.rmSync(DIST, { recursive: true, force: true });
  fs.mkdirSync(DIST, { recursive: true });

  collectPages();
  const pages = new Map();
  for (const src of PAGE_SOURCES.keys()) pages.set(src, renderPage(src));
  for (const [src, p] of pages) if (!p.title) fail(`${src} has no '# ' title`);

  const nav = guideNav(pages);
  for (const [src, p] of pages) {
    let content = p.html;
    const os = src.match(/^docs\/install\/(macos|windows|linux)\.md$/)?.[1];
    if (os) {
      // The OS's download buttons, right under the page title.
      content = content.replace(/(<\/h1>\n)/, `$1${downloadBlock("../", { os, heading: "Download", id: "download" })}\n`);
    }
    docsPage({
      out: PAGE_SOURCES.get(src),
      nav,
      title: navTitle(p.title) === p.title ? p.title : `Install on ${navTitle(p.title)}`,
      description: p.description.slice(0, 200),
      content,
      current: src.startsWith("docs/install/") ? "install" : "guide",
      sourceRel: src,
    });
  }
  installIndex(nav);
  await homePage(pages);

  // Anchors: every #fragment a docs page links must exist on its target.
  const anchorsByOut = new Map([...pages.values()].map((p) => [p.out, p.anchors]));
  for (const l of ANCHOR_LINKS) {
    const set = anchorsByOut.get(l.page);
    if (set && !set.has(l.anchor)) fail(`${l.from}: link to ${l.page}#${l.anchor}, which has no such heading`);
  }

  // Design system, as-is.
  const DS = "app/src/styles/ds";
  copy(`${DS}/tokens.css`, "ds/tokens.css");
  copy(`${DS}/components.css`, "ds/components.css");
  for (const f of fs.readdirSync(path.join(ROOT, DS, "fonts"))) copy(`${DS}/fonts/${f}`, `ds/fonts/${f}`);

  // Site assets.
  for (const f of ["site.css", "site.js", "logo.svg"]) copy(`site/src/${f}`, `assets/${f}`);
  for (const [from, to] of COPIES) copy(from, to);
  fs.writeFileSync(path.join(DIST, ".nojekyll"), "");

  const files = [];
  const walk = (d) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else files.push(fs.statSync(p).size);
    }
  };
  walk(DIST);
  const total = files.reduce((a, b) => a + b, 0);
  console.log(`build: ${pages.size + 2} pages, ${files.length} files, ${(total / 1e6).toFixed(1)} MB in site/dist`);
}

main().catch((e) => fail(e.stack || e));
