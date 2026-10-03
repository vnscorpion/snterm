// Chụp màn hình giao diện chạy ở chế độ mock (trình duyệt), để so bố cục với v1.
import { chromium } from 'playwright-core';
import { spawn } from 'node:child_process';
import { mkdirSync } from 'node:fs';

const root = '/home/user/snterm/v2/crates/app-tauri';
const out = '/home/user/snterm/v2/crates/app-tauri/scripts/out';
mkdirSync(out, { recursive: true });
const server = spawn('npx', ['vite', 'preview', '--port', '4173', '--strictPort'], { cwd: root, stdio: 'pipe' });
await new Promise((r) => setTimeout(r, 2500));
const browser = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium-1194/chrome-linux/chrome', args: ['--no-sandbox'] });
try {
  const page = await browser.newPage({ viewport: { width: 1100, height: 700 } });
  page.on('pageerror', (e) => console.log('PAGEERROR', e.message));
  page.on('console', (m) => { if (m.type() === 'error') console.log('CONSOLE', m.text()); });
  await page.goto('http://localhost:4173/');
  await page.waitForSelector('.toolbar');
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${out}/01-main-dark.png` });

  // Mở tab: nhấp đúp VM đầu tiên
  await page.dblclick('.row >> nth=0');
  await page.waitForTimeout(1200);
  await page.keyboard.type('ls -la');
  await page.screenshot({ path: `${out}/02-terminal-sftp.png` });

  // Sessions tab + chọn nhiều + menu chuột phải
  await page.click('.ltab >> nth=0');
  await page.click('.row >> nth=1');
  await page.click('.row >> nth=2', { modifiers: ['Control'] });
  await page.click('.row >> nth=2', { button: 'right' });
  await page.waitForTimeout(200);
  await page.screenshot({ path: `${out}/03-context-menu.png` });
  await page.keyboard.press('Escape');

  // Form Thêm VM
  await page.click('.toolbar .btn >> nth=1');
  await page.waitForSelector('.modal');
  await page.screenshot({ path: `${out}/04-add-vm.png` });
  await page.keyboard.press('Escape');

  // Cài đặt → Light
  await page.click('.toolbar .btn >> nth=4');
  await page.waitForSelector('.modal');
  await page.screenshot({ path: `${out}/05-settings.png` });
  await page.selectOption('#s-theme', 'Light');
  await page.selectOption('#s-lang', 'en');
  await page.click('.modal-actions .btn.primary');
  await page.waitForTimeout(400);
  await page.screenshot({ path: `${out}/06-main-light-en.png` });

  // Export / Import
  await page.click('.toolbar .btn >> nth=2'); await page.waitForSelector('.modal'); await page.screenshot({ path: `${out}/07-export.png` }); await page.keyboard.press('Escape');
  await page.click('.toolbar .btn >> nth=3'); await page.waitForSelector('.modal'); await page.waitForTimeout(300); await page.screenshot({ path: `${out}/08-import.png` }); await page.keyboard.press('Escape');
  // Trở lại Dark/vi
  await page.click('.toolbar .btn >> nth=4'); await page.selectOption('#s-theme', 'Dark'); await page.selectOption('#s-lang', 'vi'); await page.click('.modal-actions .btn.primary');
  console.log('screenshots ok');
} finally {
  await browser.close();
  server.kill();
}
