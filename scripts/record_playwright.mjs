import { chromium } from "playwright";
import fs from "fs";
import path from "path";

async function main() {
  const outDir = "/Users/ozone/projects/afterswap/docs/playwright_frames";
  fs.mkdirSync(outDir, { recursive: true });

  console.log("Launching Chromium...");
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 1.5,
  });

  const page = await context.newPage();
  let frameCount = 0;

  async function snap(name) {
    const p = path.join(outDir, `frame_${String(frameCount++).padStart(5, "0")}.png`);
    await page.screenshot({ path: p, fullPage: false });
  }

  console.log("Navigating to https://afterswap.solana-thailand.workers.dev/?replay");
  await page.goto("https://afterswap.solana-thailand.workers.dev/?replay");
  await page.waitForTimeout(2000);

  // 1. Dashboard initial state (Frames 0-30)
  for (let i = 0; i < 30; i++) {
    await snap("dashboard_init");
    await page.waitForTimeout(60);
  }

  // 2. Click Open Position (Frames 30-90)
  console.log("Opening position...");
  await page.click("#open");
  for (let i = 0; i < 60; i++) {
    await snap("trading_live");
    await page.waitForTimeout(100);
  }

  // 3. Hover over price chart & FSM (Frames 90-140)
  console.log("Hovering chart & inspecting FSM...");
  await page.hover("#chart");
  for (let i = 0; i < 50; i++) {
    await snap("chart_hover");
    await page.waitForTimeout(80);
  }

  // 4. Scroll down to Pareto, Leaderboard, Scoreboard (Frames 140-200)
  console.log("Scrolling to Pareto & Scoreboard...");
  await page.evaluate(() => window.scrollBy({ top: 500, behavior: "smooth" }));
  for (let i = 0; i < 60; i++) {
    await snap("scoreboard");
    await page.waitForTimeout(80);
  }

  // 5. Scroll back up and navigate to /rail (Frames 200-240)
  console.log("Navigating to /rail...");
  await page.evaluate(() => window.scrollTo({ top: 0, behavior: "smooth" }));
  await page.waitForTimeout(500);
  await page.click("a.navlink");
  await page.waitForTimeout(1500);

  // 6. View Rail table (Frames 240-280)
  for (let i = 0; i < 40; i++) {
    await snap("rail_table");
    await page.waitForTimeout(80);
  }

  // 7. Click on first receipt row to open modal (Frames 280-350)
  console.log("Opening execution receipt modal...");
  const firstRow = await page.$("tr.rec");
  if (firstRow) {
    await firstRow.click();
    await page.waitForTimeout(1000);
    for (let i = 0; i < 70; i++) {
      await snap("receipt_modal");
      await page.waitForTimeout(80);
    }
  }

  console.log(`Successfully captured ${frameCount} frames!`);
  await browser.close();
}

main().catch(console.error);
