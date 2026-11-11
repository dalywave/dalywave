// Сервер PO-токенов bgutil для загрузчика DalyWave: тот же src/main.ts, но живёт, пока жив
// загрузчик. Загрузчик держит stdin сервера открытым; закрылся (загрузчик вышел, упал или убит
// SIGKILL) — выходим и мы, сиротой сервер не остаётся. Кладётся сборкой в bgutil-server/.
(async () => {
    try {
        for await (const _ of Deno.stdin.readable) {
            // данные не нужны — ждём конца потока
        }
    } catch {
        // stdin уже закрыт
    }
    Deno.exit(0);
})();

await import("./src/main.ts");
