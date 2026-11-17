// Captura screenshots reais do cockpit preview (Cloudflare Pages).
// Modo 1: offline real (gateway não deployado).
// Modo 2: /api/v1/health interceptado (mock) para renderizar as tabs; demais APIs ficam sem resposta real.
const { chromium } = require('playwright-core');
const fs = require('fs');
const path = require('path');
const URL = 'https://fix-ci-trigger-master.zylos-cockpit-preview.pages.dev';
const OUT = process.argv[2];
const CHROME = 'C:/Program Files/Google/Chrome/Application/chrome.exe';

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const browser = await chromium.launch({ executablePath: CHROME, headless: true, args: ['--disable-gpu'] });
  const log = [];

  // 1) Estado offline real
  {
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    const page = await ctx.newPage();
    await page.goto(URL, { waitUntil: 'networkidle', timeout: 60000 });
    await page.waitForTimeout(4000);
    await page.screenshot({ path: path.join(OUT, '00-offline-gateway.png') });
    log.push({ file: '00-offline-gateway.png', text: (await page.innerText('body')).slice(0, 300) });
    await ctx.close();
  }

  // 2) Health mockado -> tabs
  {
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    await ctx.route('http://localhost:3000/**', async (route) => {
      const u = route.request().url();
      if (u.endsWith('/api/v1/health')) {
        return route.fulfill({ status: 200, contentType: 'application/json',
          headers: { 'access-control-allow-origin': '*' },
          body: JSON.stringify({ status: 'ok', active_streams: 0, version: 'preview-mock' }) });
      }
      return route.abort('connectionrefused');
    });
    const page = await ctx.newPage();
    await page.goto(URL, { waitUntil: 'networkidle', timeout: 60000 });
    await page.waitForSelector('.tab-btn', { timeout: 30000 });
    await page.waitForTimeout(2000);
    const tabs = await page.$$eval('.tab-btn', bs => bs.map(b => ({ cls: [...b.classList].find(c => c.startsWith('tab-') && c !== 'tab-btn'), label: b.innerText.trim() })));
    for (let i = 0; i < tabs.length; i++) {
      const t = tabs[i];
      await page.click(`.tab-btn.${t.cls}`);
      await page.waitForTimeout(1500);
      const name = `${String(i + 1).padStart(2, '0')}-${t.cls.replace(/^tab-/, '')}.png`;
      await page.screenshot({ path: path.join(OUT, name) });
      const main = await page.$('.cockpit-main');
      log.push({ file: name, label: t.label, text: main ? (await main.innerText()).replace(/\s+/g, ' ').slice(0, 220) : '' });
    }
    await ctx.close();
  }
  await browser.close();
  fs.writeFileSync(path.join(OUT, '..', 'shots-log.json'), JSON.stringify(log, null, 1));
  console.log(JSON.stringify(log, null, 1));
})().catch(e => { console.error(e); process.exit(1); });
