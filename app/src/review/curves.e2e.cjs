// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5613/";

async function open(width = 1440, height = 900) {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width, height } });
    await page.goto(BASE);
    await page.waitForSelector(".gallery");
    return { browser, page };
  } catch (err) {
    await browser.close();
    throw err;
  }
}

async function pick(page, title) {
  await page.locator(".gallery .card", { hasText: title }).first().click();
  await page.waitForSelector(".gallery", { state: "detached" });
  await page.waitForTimeout(600);
}

const inspector = (page) => page.locator(".panel-right");
const viewRadio = (page, name) =>
  inspector(page).locator("[role=radiogroup][aria-label=View]").getByRole("radio", { name, exact: true });
const paramValue = (page, name) => page.getByRole("textbox", { name: `Value of ${name}`, exact: true }).inputValue();

async function branchSettled(page) {
  await page.waitForFunction(
    () => {
      const list = document.querySelector("[data-branches]");
      return !!list && !list.textContent.includes("Computing…") && document.querySelectorAll("[data-special-kind]").length > 0;
    },
    null,
    { timeout: 30000 },
  );
}

async function curvesSettled(page) {
  await page.waitForFunction(
    () => {
      const list = document.querySelector("[data-curves]");
      return !!list && !list.querySelector("[data-curve-status=computing]") && !document.querySelector("[data-curves-search]");
    },
    null,
    { timeout: 30000 },
  );
}

test("CO oxidation: from an H row in the Branch view to a BT row of the Parameter plane, which sets both parameters", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "CO oxidation");
    await viewRadio(page, "Continuation").click();
    await branchSettled(page);
    const kinds = await inspector(page)
      .locator("[data-special-points] [data-special-kind]")
      .evaluateAll((rows) => rows.map((r) => r.dataset.specialKind));
    assert.deepEqual(kinds, ["hopf", "fold", "hopf", "fold"], "H, LP, H, LP in increasing k₂");

    const row = inspector(page).locator("[data-special-kind=hopf]").last();
    await row.getByRole("button", { name: "Follow in two parameters…" }).click();
    const panel = row.getByRole("group", { name: "Follow this Hopf point in two parameters" });
    await panel.getByLabel("Free also").selectOption({ label: "K" });
    assert.equal(await panel.locator("[data-follow-note]").innerText(), "Opens the Parameter plane with k₂ across and K up, from 0 to 2.");
    await panel.getByRole("button", { name: "Follow", exact: true }).click();

    assert.equal(await viewRadio(page, "Parameter plane").getAttribute("aria-checked"), "true");
    await curvesSettled(page);
    assert.equal(
      await inspector(page).locator("[data-plane-caption]").innerText(),
      "Across is always the Branch view’s parameter, so the two views line up.",
    );
    const curveRow = inspector(page).locator("[data-curves] > li");
    assert.equal(await curveRow.count(), 1);
    assert.match(await curveRow.innerText(), /Hopf curve[\s\S]*From H at k₂ = 1\.0516, K = 0\.4[\s\S]*534 points · stopped at the range's end · neutral past the BT points/);
    const points = inspector(page).locator("[data-plane-points] [data-plane-point]");
    assert.deepEqual(await points.evaluateAll((rows) => rows.map((r) => r.dataset.planePoint)), [
      "bogdanovTakens",
      "bogdanovTakens",
      "bautin",
      "bautin",
    ]);
    assert.match(await page.locator(".legend-bar").innerText(), /Hopf \(H\)[\s\S]*No bifurcation \(neutral saddle\)[\s\S]*Branch view’s range, K = 0\.4/);

    const bt = points.first();
    assert.match(await bt.innerText(), /BT Bogdanov–Takens\s+k₂ = 1\.161199 {2}K = 0\.722339/);
    await bt.getByRole("button").first().click();
    await page.waitForTimeout(200);
    assert.match(await paramValue(page, "k_2"), /^1\.161/);
    assert.match(await paramValue(page, "K"), /^0\.7223/);
    assert.match(await page.locator(".legend-bar").innerText(), /Branch view’s range, K = 0\.4\n/, "the slice stays at the K the branches were computed at");
  } finally {
    await browser.close();
  }
});

test("Search in two parameters fills the list curve by curve and merges the repeated fold and Hopf curves", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "CO oxidation");
    await viewRadio(page, "Continuation").click();
    await branchSettled(page);
    await inspector(page).getByRole("button", { name: "Search in two parameters" }).click();
    await curvesSettled(page);
    const rows = await inspector(page)
      .locator("[data-curves] > li")
      .evaluateAll((els) => els.map((el) => [el.dataset.curveKind, el.dataset.curveStatus]));
    assert.deepEqual(rows, [
      ["hopf", "done"],
      ["fold", "done"],
      ["hopf", "merged"],
      ["fold", "merged"],
    ]);
    assert.match(await inspector(page).locator("[data-curves] > li").nth(2).innerText(), /Same curve as the first: merged/);
    assert.match(await inspector(page).locator("[data-curves] > li").nth(3).innerText(), /Same curve as the second: merged/);
    assert.equal(await inspector(page).locator("[data-plane-points] [data-plane-point]").count(), 5);
    assert.match(
      await inspector(page).locator("[data-plane-point=bogdanovTakens]").first().innerText(),
      /On the fold and Hopf curves/,
    );
    const status = await page.locator(".statusbar").innerText();
    assert.match(status, /Curves \d/);
    assert.match(status, /2 curves · 960 points · 5 special/);
    assert.equal(await page.getByRole("button", { name: "Export…" }).isDisabled(), true, "the plane has no export yet");
  } finally {
    await browser.close();
  }
});

test("the plane opened with nothing to search from shows its empty state, and a one-parameter map says why it can't", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "CO oxidation");
    await viewRadio(page, "Parameter plane").click();
    const empty = page.locator(".plot-empty");
    assert.match(await empty.innerText(), /No bifurcation to follow yet/);
    await empty.getByRole("button", { name: "Go to the Branch view" }).click();
    assert.equal(await viewRadio(page, "Continuation").getAttribute("aria-checked"), "true");

    await page.goto(BASE);
    await page.waitForSelector(".gallery");
    await pick(page, "Logistic map");
    const plane = viewRadio(page, "Parameter plane");
    assert.equal(await plane.getAttribute("aria-disabled"), "true");
    const why = await plane.getAttribute("aria-describedby");
    assert.equal(await page.locator(`[id="${why}"]`).innerText(), "The parameter plane needs two parameters; Logistic map has 1.");
  } finally {
    await browser.close();
  }
});

test("the predator–prey map: Search follows the NS and branch-point curves, and the flip of F2 has none", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Predator–prey map");
    await viewRadio(page, "Continuation").click();
    await branchSettled(page);
    await inspector(page).getByRole("button", { name: "Search in two parameters" }).click();
    await curvesSettled(page);
    const rows = inspector(page).locator("[data-curves] > li");
    assert.deepEqual(await rows.evaluateAll((els) => els.map((el) => [el.dataset.curveKind, el.dataset.curveStatus])), [
      ["branchPoint", "failed"],
      ["flip", "done"],
      ["branchPoint", "failed"],
      ["branchPoint", "done"],
      ["flip", "failed"],
      ["neimarkSacker", "done"],
    ]);
    assert.match(await rows.nth(1).innerText(), /From PD at a = 0\.8182, ε = 1\n147 points · stopped at Max points and at the range's end$/);
    assert.match(await rows.nth(4).innerText(), /From PD at a = 3\.0000, ε = 1\nThe preview has no curve through this point\.$/);
    assert.equal(await inspector(page).locator(".inspector-stall").count(), 0);
    const kinds = await inspector(page)
      .locator("[data-plane-points] [data-plane-point]")
      .evaluateAll((els) => els.map((el) => el.dataset.planePoint));
    assert.deepEqual(kinds, ["resonance2", "resonance3", "resonance4", "chenciner", "foldFlip", "generalizedFlip"]);
  } finally {
    await browser.close();
  }
});

test("the predator–prey map: a BP row offers Switch branch and Follow, and its curve passes the fold-flip", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Predator–prey map");
    await viewRadio(page, "Continuation").click();
    await branchSettled(page);
    const bp = inspector(page).locator("[data-special-kind=branchPoint]", { hasText: "a = 1.666667" });
    assert.equal(await bp.getByRole("button", { name: "Switch branch" }).count(), 1);
    await bp.getByRole("button", { name: "Follow in two parameters…" }).click();
    const panel = bp.getByRole("group", { name: "Follow this branch point in two parameters" });
    assert.equal(await panel.locator("[data-follow-note]").innerText(), "Opens the Parameter plane with a across and ε up, from −6 to 3.");
    await panel.getByRole("button", { name: "Follow", exact: true }).click();

    assert.equal(await viewRadio(page, "Parameter plane").getAttribute("aria-checked"), "true");
    await curvesSettled(page);
    const curveRows = inspector(page).locator("[data-curves] > li");
    assert.equal(await curveRows.count(), 1);
    assert.match(await curveRows.innerText(), /Branch-point curve[\s\S]*From BP at a = 1\.6667, ε = 1[\s\S]*350 points · stopped at the range's end/);
    assert.match(await page.locator(".legend-bar").innerText(), /^Branch point \(BP\)\n/);
    const lppd = inspector(page).locator("[data-plane-points] [data-plane-point]");
    assert.deepEqual(await lppd.evaluateAll((rows) => rows.map((r) => r.dataset.planePoint)), ["foldFlip"]);
    assert.match(await lppd.innerText(), /On the branch-point curve$/);

    await viewRadio(page, "Continuation").click();
    await branchSettled(page);
    const pd = inspector(page).locator("[data-special-kind=flip]", { hasText: "a = 3.000000" });
    await pd.getByRole("button", { name: "Follow in two parameters…" }).click();
    await pd.getByRole("group", { name: "Follow this flip in two parameters" }).getByRole("button", { name: "Follow", exact: true }).click();
    await curvesSettled(page);
    assert.deepEqual(await curveRows.evaluateAll((els) => els.map((el) => [el.dataset.curveKind, el.dataset.curveStatus])), [
      ["branchPoint", "done"],
      ["flip", "failed"],
    ]);
    assert.match(await page.locator(".legend-bar").innerText(), /^Branch point \(BP\)\n/);
    assert.match(await lppd.innerText(), /^LPPD Fold-flip\na = 3\.000000 {2}ε = 2\.000000\nx = 0\.6667, y = 0\.0000\nA multiplier \+1 and a multiplier −1 together\. Here the \+1 is a branch point, where another branch of fixed points crosses, so there is no fold\.\nOn the branch-point curve$/);
  } finally {
    await browser.close();
  }
});

/** The template's Parameter plane after Search in two parameters has followed every curve. */
async function searchPlane(page, title) {
  await pick(page, title);
  await viewRadio(page, "Continuation").click();
  await branchSettled(page);
  await inspector(page).getByRole("button", { name: "Search in two parameters" }).click();
  await curvesSettled(page);
}

const rowOf = (page, kind) => inspector(page).locator(`[data-plane-point=${kind}]`);
const sentence = (row) => row.locator("[data-plane-sentence]").innerText();
const coefficients = (row) => row.locator("[data-plane-coefficients]").innerText();

test("CO oxidation: each special point says what happens there, with its coefficients, and the inset shows the crosshair", async () => {
  const { browser, page } = await open();
  try {
    await searchPlane(page, "CO oxidation");

    const bt = rowOf(page, "bogdanovTakens");
    assert.equal(await bt.count(), 2);
    assert.equal(await sentence(bt.first()), "A fold curve and a Hopf curve meet. A curve of homoclinic orbits starts here.");
    assert.equal(await coefficients(bt.first()), "a = 0.0482, b = 1.9376 · ab > 0", "MatCont: |a| = 0.04822563, |b| = 1.937632");
    assert.equal(await coefficients(bt.last()), "a = 0.0838, b = 2.1363 · ab > 0", "MatCont: |a| = 0.08378442, |b| = 2.136280");
    const cp = rowOf(page, "cusp");
    assert.equal(await sentence(cp), "Two fold curves meet. Inside the wedge there are three fixed points, outside one.");
    assert.equal(await coefficients(cp), "c = 0.3628", "MatCont: 0.3627844");
    const gh = rowOf(page, "bautin");
    assert.equal(await gh.locator("[data-plane-coefficients]").count(), 0, "ℓ₂ is not computed");
    assert.deepEqual(
      await gh.locator("[data-plane-sentence]").allInnerTexts(),
      Array(2).fill("The Hopf bifurcation changes from subcritical (right) to supercritical (left)."),
      "the Hopf points at K = 0.4, right of both, are subcritical",
    );

    const inset = page.locator("[data-plane-inset]");
    assert.equal(await inset.getAttribute("aria-label"), "Live inset: phase portrait at the crosshair");
    await page.waitForFunction(() => document.querySelector("[data-plane-inset]:not(.is-stale)"));
    assert.match(await inset.locator("figcaption").innerText(), /^k₂ = 1\.050, K = 0\.400\n(One fixed point in view|\d fixed points in view|No fixed point in view)/);

    await bt.first().getByRole("button", { name: /^Bogdanov–Takens: / }).click();
    await page.waitForFunction(() => /K = 0\.722/.test(document.querySelector("[data-plane-inset]:not(.is-stale) figcaption")?.textContent ?? ""));
  } finally {
    await browser.close();
  }
});

test("the predator–prey map: weak-resonance ticks, the multipliers' angle, a fold-flip that promises no fold, and the orbit inset", async () => {
  const { browser, page } = await open();
  try {
    await searchPlane(page, "Predator–prey map");

    assert.match(await page.locator(".legend-help").innerText(), /^Ticks 1:5 to 1:9 mark where the rotation is a rational turn: each is the tip of a resonance tongue\./);
    assert.match(await rowOf(page, "resonance4").innerText(), /x = 0\.4286, y = 1\.5000 · θ = 90°/);
    assert.match(await rowOf(page, "resonance3").innerText(), /· θ = 120°/);
    const lppd = rowOf(page, "foldFlip");
    assert.equal(
      await sentence(lppd),
      "A multiplier +1 and a multiplier −1 together. Here the +1 is a branch point, where another branch of fixed points crosses, so there is no fold.",
    );
    assert.equal(
      await sentence(rowOf(page, "chenciner")),
      "The Neimark–Sacker bifurcation changes from supercritical (above) to subcritical (below).",
      "the NS curve runs almost straight down through CH, and d > 0 below it",
    );

    const inset = page.locator("[data-plane-inset]");
    assert.equal(await inset.getAttribute("aria-label"), "Live inset: orbit at the crosshair");
    await page.waitForFunction(() => document.querySelector("[data-plane-inset]:not(.is-stale)"));
    assert.match(await inset.locator("figcaption").innerText(), /^a = 5\.300, ε = 1\.000\n.*The orbits? (do|does) not repeat within 3,000 iterates\./s);
  } finally {
    await browser.close();
  }
});

const captured = [];

for (const [title, template, param, upName, key] of [
  ["CO oxidation", "co-oxidation", "K", "K", "ArrowRight"],
  ["Predator–prey map", "predator-prey-map", "varepsilon", "ε", "ArrowLeft"],
]) {
  test(`${title}: dragging a parameter coalesces the inset's requests and ends on the last value`, async () => {
    const { browser, page } = await open();
    try {
      await searchPlane(page, title);
      await page.waitForFunction(() => document.querySelector("[data-plane-inset]:not(.is-stale)"));
      await page.evaluate(() => {
        const mock = window.__sepxMock;
        let last = mock.lastPortrait;
        const log = (window.__insetLog = [{ request: last.req, elapsedMs: last.res.elapsedMs }]);
        Object.defineProperty(mock, "lastPortrait", {
          get: () => last,
          set: (v) => {
            last = v;
            log.push({ request: v.req, elapsedMs: v.res.elapsedMs });
          },
        });
      });
      const slider = page.getByRole("slider", { name: `Parameter ${param}`, exact: true });
      await slider.focus();
      const presses = 40;
      for (let i = 0; i < presses; i++) await slider.press(key);
      const value = Number((await paramValue(page, param)).replace("−", "-"));
      await page.waitForFunction(
        (want) => (document.querySelector("[data-plane-inset]:not(.is-stale) figcaption")?.textContent ?? "").includes(want),
        `${upName} = ${value.toFixed(3).replace(/^-/, "−")}`,
        { timeout: 30000 },
      );
      const log = await page.evaluate(() => window.__insetLog);
      assert.ok(log.length >= 2 && log.length <= presses, `${log.length - 1} inset requests for ${presses} moves`);
      assert.equal(log[log.length - 1].request.params[1], value, "the last request is at the last value");
      const ms = log.map((l) => l.elapsedMs).sort((a, b) => a - b);
      console.log(`${title}, preview inset: ${log.length - 1} requests for ${presses} moves, median ${ms[ms.length >> 1].toFixed(1)} ms, max ${ms[ms.length - 1].toFixed(1)} ms`);
      captured.push(...log.map((l) => ({ template, request: l.request })));
      if (process.env.SEPX_INSET_REQUESTS) require("node:fs").writeFileSync(process.env.SEPX_INSET_REQUESTS, JSON.stringify(captured));
    } finally {
      await browser.close();
    }
  });
}

test("CO oxidation: the plane saved after Search and Follow homoclinic opens again as the same plane, and its Branch view comes back", async () => {
  const { browser, page } = await open();
  try {
    await searchPlane(page, "CO oxidation");
    await rowOf(page, "bogdanovTakens").first().getByRole("button", { name: "Follow homoclinic" }).click();
    await page.waitForSelector("[data-homoclinic-status=done]", { timeout: 30000 });
    const shown = () =>
      page.evaluate(() => {
        const texts = (sel) => [...document.querySelectorAll(sel)].map((el) => el.innerText);
        return {
          curves: texts(".panel-right [data-curves] > li"),
          points: texts(".panel-right [data-plane-points] [data-plane-point]"),
          homoclinic: texts(".panel-right [data-homoclinic-status]"),
          legend: document.querySelector(".legend-bar")?.innerText,
        };
      });
    const before = await shown();
    assert.equal(before.curves.length, 5, "four curves, two merged, and the homoclinic curve");
    assert.equal(before.points.length, 5);
    assert.equal(before.homoclinic.length, 1);
    assert.match(before.legend, /Homoclinic \(Hom\)[\s\S]*Branch view’s range, K = 0\.4/);
    const params = async () => [await paramValue(page, "k_2"), await paramValue(page, "K")];
    const values = await params();

    await page.locator("canvas.plot-canvas").focus();
    await page.keyboard.press("ControlOrMeta+s");
    await page.waitForFunction(() => window.__sepxMock.files.has("/mock/co-oxidation.sepx"));
    const saved = JSON.parse(await page.evaluate(() => window.__sepxMock.files.get("/mock/co-oxidation.sepx")));
    assert.equal(saved.views[0].kind, "curves");
    assert.deepEqual([saved.views[0].x_axis.name, saved.views[0].y_axis.name], ["k_2", "K"]);
    assert.deepEqual(saved.views[0].curves.seeds.map((s) => s.kind), ["hopf", "fold", "hopf", "fold"]);
    assert.equal(saved.views[0].curves.homoclinic.length, 1);

    await page.locator(".topbar").getByRole("button", { name: "Templates…", exact: true }).click();
    await page.waitForSelector(".gallery");
    await pick(page, "Lorenz");
    await page.locator(".topbar").getByRole("button", { name: "Open…", exact: true }).click();
    await page.waitForFunction(() => document.title === "co-oxidation — Separatrix");
    assert.equal(await viewRadio(page, "Parameter plane").getAttribute("aria-checked"), "true");
    await curvesSettled(page);
    await page.waitForSelector("[data-homoclinic-status=done]", { timeout: 30000 });
    assert.deepEqual(await shown(), before, "the same curves, special points, homoclinic curve and slice");
    assert.deepEqual(await params(), values);
    assert.equal(await page.locator(".project-banner").count(), 0, "nothing to report");
    assert.equal(await page.locator(".topbar .doc-dirty").count(), 0, "clean after opening");

    await viewRadio(page, "Continuation").click();
    await branchSettled(page);
    const kinds = await inspector(page)
      .locator("[data-special-points] [data-special-kind]")
      .evaluateAll((rows) => rows.map((r) => r.dataset.specialKind));
    assert.deepEqual(kinds, ["hopf", "fold", "hopf", "fold"], "the Branch view the plane came from");
  } finally {
    await browser.close();
  }
});
