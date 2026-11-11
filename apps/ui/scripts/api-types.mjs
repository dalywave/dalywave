// Типы API из OpenAPI сервера: bun run api-types (сервер должен работать на :8400 или по API_URL).
// Если в схеме сервера есть ссылка на схему, которой там нет, тип собирается со строковой заглушкой,
// а скрипт громко об этом пишет — это ошибка сервера, её нужно исправить там.
import { writeFileSync } from 'node:fs'
import openapiTS, { astToString } from 'openapi-typescript'

const base = process.env.API_URL ?? 'http://localhost:8400'
const out = new URL('../app/api/schema.d.ts', import.meta.url)

const res = await fetch(`${base}/api/openapi.json`)
if (!res.ok) throw new Error(`OpenAPI: HTTP ${res.status} от ${base}`)
const spec = await res.json()

const schemas = (spec.components ??= {}).schemas ??= {}
const dangling = new Set()
const walk = (node) => {
  if (Array.isArray(node)) return node.forEach(walk)
  if (!node || typeof node !== 'object') return
  const ref = node.$ref
  if (typeof ref === 'string' && ref.startsWith('#/components/schemas/')) {
    const name = ref.slice('#/components/schemas/'.length)
    if (!(name in schemas)) dangling.add(name)
  }
  Object.values(node).forEach(walk)
}
walk(spec)

for (const name of dangling) {
  schemas[name] = { type: 'string', description: 'Схемы нет в OpenAPI сервера — заглушка scripts/api-types.mjs' }
}
if (dangling.size) console.warn(`⚠ В схеме сервера нет: ${[...dangling].join(', ')} — подставлена строка. Исправьте на сервере.`)

// operationId должны быть уникальными; повторы переименовываются по пути
const seen = new Map()
const renamed = []
for (const [path, item] of Object.entries(spec.paths ?? {})) {
  for (const [method, op] of Object.entries(item)) {
    if (!op || typeof op !== 'object' || !op.operationId) continue
    if (seen.has(op.operationId)) {
      const unique = `${op.operationId}_${method}_${path.replace(/^\/api\//, '').replace(/[^a-z0-9]+/gi, '_')}`
      renamed.push(`${op.operationId} (${method.toUpperCase()} ${path}) → ${unique}`)
      op.operationId = unique
    }
    seen.set(op.operationId, path)
  }
}
if (renamed.length) console.warn(`⚠ Повторяющиеся operationId в схеме сервера:\n  ${renamed.join('\n  ')}`)

const ast = await openapiTS(spec)
writeFileSync(out, astToString(ast))
console.log(`✓ ${Object.keys(spec.paths ?? {}).length} путей, ${Object.keys(schemas).length} схем → app/api/schema.d.ts`)
