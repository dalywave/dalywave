// Установщик для людей: dist/rest.daly.wave.streamDeckPlugin (двойной клик — Stream Deck ставит плагин).
// Версия — VERSION (её передаёт `make release-streamdeck`, та же, что у приложения) или из package.json.
// Упаковка CLI Elgato заодно проверяет манифест, картинки и раскладки.
import { existsSync, rmSync } from 'node:fs'
import { join } from 'node:path'

const ROOT = join(import.meta.dir, '..')
const DIST = join(ROOT, 'dist')
const FILE = join(DIST, 'rest.daly.wave.streamDeckPlugin')

function run(cmd: string[]): void {
  const r = Bun.spawnSync(cmd, { cwd: ROOT, stdout: 'inherit', stderr: 'inherit', env: process.env })
  if (r.exitCode !== 0) throw new Error(`${cmd.join(' ')}: код ${r.exitCode}`)
}

run(['bun', 'scripts/build.ts'])
rmSync(FILE, { force: true })
run(['bunx', 'streamdeck', 'pack', 'rest.daly.wave.sdPlugin', '--output', DIST, '--force', '--no-update-check', '--no-file-list'])
if (!existsSync(FILE)) throw new Error(`нет ${FILE}`)
console.log(FILE)
