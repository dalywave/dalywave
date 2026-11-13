// Сборка плагина в rest.daly.wave.sdPlugin: манифест и картинки, инспекторы, профиль Stream Deck +,
// код (bin/plugin.js — один файл ESM для Node.js 24 из Stream Deck). VERSION=0.2.0 — версия выпуска.
//   bun run build            — собрать
//   bun run build --watch    — пересобирать при правках (Stream Deck перезапускает плагин сам: `streamdeck restart`)
import { mkdirSync, watch, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const ROOT = join(import.meta.dir, '..')
const PLUGIN = join(ROOT, 'rest.daly.wave.sdPlugin')
const pkg = await Bun.file(join(ROOT, 'package.json')).json() as { version: string }
const version = process.env.VERSION ?? pkg.version

function run(script: string): void {
  const r = Bun.spawnSync(['bun', join(import.meta.dir, script)], { stdout: 'inherit', stderr: 'inherit', env: { ...process.env, VERSION: version } })
  if (r.exitCode !== 0) throw new Error(`${script} упал`)
}

async function build(): Promise<boolean> {
  run('assets.ts')
  run('inspectors.ts')
  run('profile.ts')
  const out = await Bun.build({
    entrypoints: [join(ROOT, 'src', 'plugin.ts')],
    outdir: join(PLUGIN, 'bin'),
    naming: 'plugin.js',
    target: 'node',
    format: 'esm',
    minify: false,
    sourcemap: 'none',
    // Необязательные ускорители ws: без них он работает
    external: ['bufferutil', 'utf-8-validate'],
    define: { PLUGIN_VERSION: JSON.stringify(version) },
  })
  if (!out.success) {
    for (const log of out.logs) console.error(log)
    return false
  }
  mkdirSync(join(PLUGIN, 'bin'), { recursive: true })
  writeFileSync(join(PLUGIN, 'bin', 'package.json'), '{ "type": "module" }\n')
  console.log(`плагин ${version} собран`)
  return true
}

if (!(await build())) process.exit(1)

if (process.argv.includes('--watch')) {
  let timer: Timer | null = null
  watch(join(ROOT, 'src'), { recursive: true }, () => {
    if (timer) clearTimeout(timer)
    timer = setTimeout(async () => {
      if (await build()) Bun.spawnSync(['bunx', 'streamdeck', 'restart', 'rest.daly.wave'], { stdout: 'inherit' })
    }, 200)
  })
}
