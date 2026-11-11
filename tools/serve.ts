// Статика для макетов и проверок: bun tools/serve.ts <каталог> <порт>
// Без кеша, с поддержкой Range (превью аудио), index.html для каталогов.
import { resolve, join, normalize } from "node:path";
import { statSync } from "node:fs";

const root = resolve(process.argv[2] ?? ".");
const port = Number(process.argv[3] ?? 4400);

Bun.serve({
  port,
  hostname: "0.0.0.0",
  async fetch(req) {
    const url = new URL(req.url);
    const rel = normalize(decodeURIComponent(url.pathname)).replace(/^(\.\.[/\\])+/, "");
    let path = join(root, rel);
    if (!path.startsWith(root)) return new Response("forbidden", { status: 403 });
    try {
      if (statSync(path).isDirectory()) path = join(path, "index.html");
    } catch {
      return new Response("not found", { status: 404 });
    }
    const file = Bun.file(path);
    if (!(await file.exists())) return new Response("not found", { status: 404 });
    return new Response(file, { headers: { "Cache-Control": "no-store" } });
  },
});

console.log(`serving ${root} on http://localhost:${port}`);
