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
const viewGroup = (page) => inspector(page).locator("[role=radiogroup][aria-label=View]");
const viewRadio = (page, name) => viewGroup(page).getByRole("radio", { name, exact: true });
const specialRows = (page) =>
  inspector(page)
    .locator("[data-special-points] [data-special-kind]")
    .evaluateAll((rows) => rows.map((r) => [r.dataset.specialKind, r.querySelector(".inspector-special-text").textContent]));
const pOf = (text) => Number(text.split("  ")[0].split(" = ")[1].replace("−", "-"));

async function settled(page) {
  await page.waitForFunction(
    () => {
      const list = document.querySelector("[data-branches]");
      return !!list && !list.textContent.includes("Computing…") && !document.querySelector("[data-special-points]")?.textContent.includes("Computing…");
    },
    null,
    { timeout: 30000 },
  );
  await page.waitForTimeout(200);
}

test("Sel'kov: two Hopf points on b's branch, b = √0.12 and √0.72, with ω", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Sel'kov");
    await viewRadio(page, "Continuation").click();
    await inspector(page).getByLabel("Parameter", { exact: true }).selectOption({ label: "b" });
    await settled(page);
    const rows = await specialRows(page);
    assert.deepEqual(rows.map(([kind]) => kind), ["hopf", "hopf"], JSON.stringify(rows));
    assert.match(rows[0][1], /^b = 0\.346410  \(x, y\) = \(0\.3464, 1\.7321\)  ω = 0\.447214/);
    assert.match(rows[1][1], /^b = 0\.848528  \(x, y\) = \(0\.8485, 1\.0607\)  ω = 0\.894427/);
    assert.equal(await inspector(page).locator("[data-special-kind=hopf]").getByRole("button", { name: "Switch branch" }).count(), 0);
    const status = await page.locator(".statusbar").innerText();
    assert.match(status, /Continuation \d/);
    assert.match(status, /\d+ points · 2 special/);
  } finally {
    await browser.close();
  }
});

test("the logistic map: PD, then Switch branch three times, lists r1 … r4 in increasing r", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Logistic map");
    await viewRadio(page, "Continuation").click();
    await settled(page);
    for (let k = 0; k < 3; k++) {
      await inspector(page).locator("[data-special-kind=flip]").last().getByRole("button", { name: "Switch branch" }).click();
      await page.waitForFunction((n) => document.querySelectorAll("[data-branches] > li").length === n, k + 3);
      await settled(page);
    }
    const rows = await specialRows(page);
    assert.deepEqual(
      await inspector(page).locator("[data-switched-branch]").allInnerTexts(),
      ["Followed as Branch 3", "Followed as Branch 4", "Followed as Branch 5"],
      "a flip already switched at offers no second switch",
    );
    const flips = rows.filter(([kind]) => kind === "flip").map(([, text]) => pOf(text));
    assert.equal(flips.length, 4, JSON.stringify(rows));
    const known = [3, 1 + Math.sqrt(6), 3.54409036, 3.564407266];
    flips.forEach((r, k) => assert.ok(Math.abs(r - known[k]) < 2e-6, `r${k + 1} = ${r}, want ${known[k]}`));
    assert.deepEqual([...flips].sort((a, b) => a - b), flips, "in increasing r");
    assert.deepEqual(
      rows.filter(([kind]) => kind === "flip").map(([, text]) => /period (\d+)/.exec(text)?.[1] ?? "1"),
      ["1", "2", "4", "8"],
    );
    assert.equal(await page.getByRole("button", { name: /^Export/ }).isDisabled(), false, "the branch plot exports");
  } finally {
    await browser.close();
  }
});

test("a click on a special row, or on a branch, moves the slider there", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Logistic map");
    await viewRadio(page, "Continuation").click();
    await settled(page);
    const value = () => page.getByRole("textbox", { name: "Value of r", exact: true }).inputValue();
    await inspector(page).locator("[data-special-kind=branchPoint] .ds-fp-row").click();
    assert.equal(await value(), "1");
    await inspector(page).locator("[data-special-kind=flip] .ds-fp-row").click();
    assert.equal(await value(), "3");

    const canvas = page.locator(".branch-canvas");
    const box = await canvas.boundingBox();
    const x = box.x + box.width / 2;
    for (let y = box.y + 8; y < box.y + box.height - 8 && (await value()) === "3"; y += 5) await page.mouse.click(x, y);
    const v = Number(await value());
    const vertices = await page.evaluate(() => window.__sepxMock.continuations.flatMap((c) => c.res.branch.p));
    assert.ok(v !== 3 && vertices.some((p) => Number(p.toPrecision(4)) === v), `r = ${v} is a branch vertex's`);
  } finally {
    await browser.close();
  }
});

test("Continue on a fixed point row opens the Branch view from it; the selection seeds a branch too", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Sel'kov");
    const row = inspector(page).locator("[data-fp-index]").first();
    await row.locator(".ds-fp-row").click();
    await row.getByRole("button", { name: "Continue" }).click();
    assert.equal(await viewRadio(page, "Continuation").getAttribute("aria-checked"), "true");
    await settled(page);
    assert.equal(await inspector(page).locator("[data-branches] > li").count(), 1);
    assert.match(await inspector(page).locator("[data-branches]").innerText(), /From \(x, y\) = \(0\.6000, 1\.3636\) at a = 0\.0800/);
    await inspector(page).getByRole("button", { name: "Continue from selection" }).click();
    await page.waitForTimeout(200);
    assert.equal(await inspector(page).locator("[data-branches] > li").count(), 1, "the same seed is added once");
    await inspector(page).getByRole("radio", { name: "Forward" }).click();
    await inspector(page).getByRole("button", { name: "Continue from selection" }).click();
    await page.waitForFunction(() => document.querySelectorAll("[data-branches] > li").length === 2);
    await inspector(page).getByRole("button", { name: "Remove branch 2" }).click();
    assert.equal(await inspector(page).locator("[data-branches] > li").count(), 1);
  } finally {
    await browser.close();
  }
});

test("at 1,024 × 768 in projector mode every View label fits its segment, three to a row and the Parameter plane on its own", async () => {
  const { browser, page } = await open(1024, 768);
  try {
    await page.evaluate(() => (document.documentElement.dataset.theme = "light"));
    for (const [title, labels] of [
      ["Sel'kov", ["Phase", "Series", "Section", "3D", "λ", "Branch", "Parameter plane"]],
      ["Logistic map", ["Phase", "Series", "Cobweb", "λ", "Orbit", "Branch", "Parameter plane"]],
    ]) {
      if (title !== "Sel'kov") {
        await page.goto(BASE);
        await page.waitForSelector(".gallery");
        await page.evaluate(() => (document.documentElement.dataset.theme = "light"));
      }
      await pick(page, title);
      const radios = viewGroup(page).getByRole("radio");
      assert.deepEqual(await radios.allInnerTexts(), labels);
      const fits = await radios.evaluateAll((els) =>
        els.map((el) => {
          const cs = getComputedStyle(el);
          const pad = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
          const range = document.createRange();
          range.selectNodeContents(el);
          return range.getBoundingClientRect().width + pad <= el.getBoundingClientRect().width + 0.5;
        }),
      );
      assert.deepEqual(fits, [true, true, true, true, true, true, true], title);
      await viewRadio(page, "Continuation").click();
      const button = inspector(page).getByRole("button", { name: "Continue from selection" });
      assert.ok(
        await button.evaluate((el) => el.scrollWidth <= el.clientWidth + 0.5),
        `${title}: the Continue label fits its button`,
      );
      const rows = await radios.evaluateAll((els) => els.map((el) => Math.round(el.getBoundingClientRect().top)));
      assert.equal(new Set(rows).size, 3, `three rows: ${rows}`);
      const widths = await radios.evaluateAll((els) => els.map((el) => Math.round(el.getBoundingClientRect().width)));
      const thirds = widths.slice(0, 6);
      assert.ok(Math.max(...thirds) - Math.min(...thirds) <= 1, `equal segments: ${widths}`);
      assert.ok(widths[6] >= 3 * thirds[0] - 2, `the Parameter plane spans its row: ${widths}`);
    }
  } finally {
    await browser.close();
  }
});
