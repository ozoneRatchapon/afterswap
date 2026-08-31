import { chromium } from "playwright";
import fs from "fs";
import path from "path";

async function main() {
  const outDir = "/Users/ozone/projects/afterswap/docs/playwright_v2_frames";
  fs.mkdirSync(outDir, { recursive: true });

  console.log("Launching Chromium with custom cursor & perfect pacing...");
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 1.5,
  });

  const page = await context.newPage();
  let frameCount = 0;

  // Inject visible custom mouse pointer for clear visual tracking
  async function injectCursor() {
    await page.evaluate(() => {
      if (document.getElementById("custom-cursor")) return;
      const c = document.createElement("div");
      c.id = "custom-cursor";
      c.style.position = "fixed";
      c.style.width = "18px";
      c.style.height = "18px";
      c.style.borderRadius = "50%";
      c.style.background = "rgba(57, 135, 229, 0.9)";
      c.style.border = "2px solid #ffffff";
      c.style.boxShadow = "0 0 10px rgba(57, 135, 229, 0.8)";
      c.style.pointerEvents = "none";
      c.style.zIndex = "999999";
      c.style.transform = "translate(-50%, -50%)";
      c.style.transition = "transform 0.05s ease-out, background 0.1s ease";
      document.body.appendChild(c);

      window.moveCursorTo = (x, y) => {
        c.style.left = `${x}px`;
        c.style.top = `${y}px`;
      };
    });
  }

  async function moveMouseSmooth(fromX, fromY, toX, toY, steps = 12) {
    for (let i = 1; i <= steps; i++) {
      const curX = fromX + (toX - fromX) * (i / steps);
      const curY = fromY + (toY - fromY) * (i / steps);
      await page.evaluate(({ x, y }) => window.moveCursorTo && window.moveCursorTo(x, y), { x: curX, y: curY });
      await snap();
      await page.waitForTimeout(30);
    }
  }

  async function snap() {
    const p = path.join(outDir, `frame_${String(frameCount++).padStart(5, "0")}.png`);
    await page.screenshot({ path: p, fullPage: false });
  }

  console.log("1. Loading Dashboard (0:00 - 0:13)...");
  await page.goto("https://afterswap.solana-thailand.workers.dev/?replay");
  await page.waitForTimeout(1500);
  await injectCursor();

  // Move mouse to Open button
  await moveMouseSmooth(720, 450, 180, 260, 15);
  for (let i = 0; i < 20; i++) await snap();

  console.log("2. Opening Position & Trading Live (0:13 - 0:40)...");
  await page.click("#open");
  await page.evaluate(() => {
    const c = document.getElementById("custom-cursor");
    if (c) c.style.background = "rgba(217, 89, 38, 0.9)";
  });

  // Watch chart updates & move mouse to inspect active FSM
  await moveMouseSmooth(180, 260, 500, 420, 20); // Move over chart
  for (let i = 0; i < 25; i++) {
    await snap();
    await page.waitForTimeout(80);
  }

  await moveMouseSmooth(500, 420, 1050, 450, 20); // Move over FSM machine
  for (let i = 0; i < 25; i++) {
    await snap();
    await page.waitForTimeout(80);
  }

  // Scroll down smoothly to show Pareto & Scoreboard
  console.log("3. Scrolling to Lattice & Pareto (0:40)...");
  await page.evaluate(() => window.scrollBy({ top: 480, behavior: "smooth" }));
  await page.waitForTimeout(400);
  for (let i = 0; i < 25; i++) await snap();

  console.log("4. Navigating to /rail Receipts Verifier (0:40 - 1:10)...");
  await page.evaluate(() => window.scrollTo({ top: 0, behavior: "smooth" }));
  await page.waitForTimeout(300);
  await moveMouseSmooth(1050, 450, 1100, 30, 15); // Move to Receipts link
  await page.click("a.navlink");
  await page.waitForTimeout(1200);
  await injectCursor();

  // Show Rail Table
  await moveMouseSmooth(1100, 30, 400, 250, 15);
  for (let i = 0; i < 25; i++) await snap();

  console.log("5. Opening Execution Receipt Modal & Inspecting Seals (0:55 - 1:15)...");
  const firstRow = await page.$("tr.rec");
  if (firstRow) {
    await firstRow.click();
    await page.waitForTimeout(800);
    await injectCursor();
    
    // Move mouse over 4 seals inside modal
    await moveMouseSmooth(400, 250, 720, 320, 15); // Seal 1: Market
    for (let i = 0; i < 15; i++) await snap();
    
    await moveMouseSmooth(720, 320, 720, 430, 15); // Seal 2: Quote Signature
    for (let i = 0; i < 15; i++) await snap();
    
    await moveMouseSmooth(720, 430, 720, 540, 15); // Seal 3: Attestation & PDA
    for (let i = 0; i < 15; i++) await snap();
    
    // Close modal
    const closeBtn = await page.$("dialog.receipt button");
    if (closeBtn) {
      await closeBtn.click();
      await page.waitForTimeout(500);
    }
  }

  console.log("6. Returning to Dashboard to Show Radical Honesty & Benchmarks (1:15 - 1:38)...");
  await page.click("a[href='/']");
  await page.waitForTimeout(1000);
  await injectCursor();

  // Scroll directly to "Proof (not marketing)" & Scoreboard
  await page.evaluate(() => window.scrollTo({ top: 900, behavior: "smooth" }));
  await page.waitForTimeout(500);
  await moveMouseSmooth(720, 540, 450, 500, 20); // Hover over Proof & 11 Assets benchmark
  for (let i = 0; i < 40; i++) {
    await snap();
    await page.waitForTimeout(60);
  }

  console.log(`Total captured frames: ${frameCount}`);
  await browser.close();
}

main().catch(console.error);
