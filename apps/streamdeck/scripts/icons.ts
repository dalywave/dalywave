// PNG значка плагина (Stream Deck требует PNG 256 и @2x 512) из imgs/plugin/icon-source.svg.
// Нужен Chrome; запускается при смене значка, PNG лежат в репозитории. Запуск: bun scripts/icons.ts
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { chromium } from 'playwright-core'

const DIR = join(import.meta.dir, '..', 'rest.daly.wave.sdPlugin', 'imgs', 'plugin')
const svg = readFileSync(join(DIR, 'icon-source.svg'), 'utf8')

const browser = await chromium.launch({ channel: 'chrome', headless: true })
for (const [size, name] of [[256, 'marketplace.png'], [512, 'marketplace@2x.png']] as const) {
  const page = await browser.newPage({ viewport: { width: size, height: size }, deviceScaleFactor: 1 })
  await page.setContent(`<html><body style="margin:0;background:transparent">${svg.replace(/width="\d+" height="\d+"/, `width="${size}" height="${size}"`)}</body></html>`)
  await page.screenshot({ path: join(DIR, name), omitBackground: true, clip: { x: 0, y: 0, width: size, height: size } })
  await page.close()
}
await browser.close()
console.log('значок плагина: marketplace.png, marketplace@2x.png')
