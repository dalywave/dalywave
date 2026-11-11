"""Командная строка `fetcher`."""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any

from . import tools
from .config import Settings


def _print(data: Any) -> None:
    json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")


def _label_from(out: Path) -> str:
    return out.stem.removeprefix("results-") or "run"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="fetcher", description="DalyWave: поиск, скачивание и проверка аудио треков Deezer"
    )
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("match", help="кандидаты на YouTube Music для трека Deezer (JSON)")
    p.add_argument("track_id", type=int)
    p.add_argument("--videos", choices=["auto", "always", "never"], default="auto")

    p = sub.add_parser("acquire", help="поиск → скачивание → проверка (JSON)")
    p.add_argument("track_id", type=int)
    p.add_argument("--out", type=Path, required=True, help="папка для файлов")
    p.add_argument("--max-attempts", type=int, default=3)
    p.add_argument("--prefer", nargs="*", default=[], metavar="VIDEO_ID")
    p.add_argument("--exclude", nargs="*", default=[], metavar="VIDEO_ID")
    p.add_argument("--url", help="качать эту ссылку (SoundCloud, Bandcamp…) вместо найденного")
    p.add_argument("--keep-rejected", action="store_true", help="не удалять отклонённые файлы")

    p = sub.add_parser("bench", help="прогон эталонного набора")
    p.add_argument("--set", dest="set_path", type=Path, default=Path("bench/set.json"))
    p.add_argument("--out", type=Path, required=True, help="bench/results-<метка>.json")
    p.add_argument("--label", help="метка прогона (по умолчанию из имени --out)")
    p.add_argument("--audio-dir", type=Path, default=Path("bench/out"))
    p.add_argument("--delay", type=float, default=5.0, help="пауза между треками, с")
    p.add_argument("--only", nargs="*", type=int, default=None, metavar="DEEZER_ID")

    p = sub.add_parser("calibrate", help="порог отпечатков по файлам прогонов")
    p.add_argument("--audio-dir", type=Path, nargs="+", required=True, help="bench/out/<метка>")
    p.add_argument("--hard", type=Path, help="bench/hard-negatives.json — другие записи песен")
    p.add_argument("--hard-dir", type=Path, default=Path("bench/out/hard"))
    p.add_argument("--out", type=Path)

    p = sub.add_parser("serve", help="HTTP API")
    p.add_argument("--host", default="0.0.0.0")
    p.add_argument("--port", type=int, default=8402)

    p = sub.add_parser("worker", help="домашний загрузчик: задания с сервера")
    p.add_argument("--server", required=True, help="https://wave.daly.rest")
    p.add_argument("--token", default=os.environ.get("FETCHER_WORKER_TOKEN"))
    p.add_argument("--worker-id")
    p.add_argument("--once", action="store_true", help="одно задание и выход")
    p.add_argument("--poll", type=float, default=30.0, help="опрос без заданий, с")

    from . import sidecar

    p = sub.add_parser(
        "sidecar",
        help="загрузчик внутри приложения: события JSON в stdout, команды из stdin (SIDECAR.md)",
    )
    sidecar.add_arguments(p)

    p = sub.add_parser("pot-check", help="поднять свой сервис PO-токенов и получить токен (JSON)")
    sidecar.add_pot_check_arguments(p)

    args = parser.parse_args(argv)
    if args.command == "sidecar":
        code = sidecar.run(args)
        # Не ждём потоков поиска и yt-dlp: задание уже возвращено серверу
        os._exit(code)
    if args.command == "pot-check":
        result = sidecar.pot_check(args)
        _print(result)
        return 0 if result.get("ok") else 1
    if (here := tools.executable_dir()) is not None:
        # Собранный загрузчик берёт свои ffmpeg, fpcalc и deno, а не из PATH
        tools.configure({name: path for name, path in tools.resolve([here]).items() if path})
    settings = Settings.from_env()

    if args.command == "serve":
        from .server import run

        run(args.host, args.port)
        return 0

    if args.command == "calibrate":
        from .bench import calibrate, fetch_hard_negatives

        hard = None
        if args.hard:
            from .acquire import Fetcher

            hard = fetch_hard_negatives(Fetcher(settings), args.hard, args.hard_dir)
        report = calibrate(args.audio_dir, hard)
        if args.out:
            args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        _print({k: v for k, v in report.items() if not k.endswith("Pairs")})
        return 0

    from .acquire import Fetcher

    fetcher = Fetcher(settings)

    if args.command == "match":
        _print(fetcher.match(args.track_id, videos=args.videos).to_json())
        return 0

    if args.command == "acquire":
        result = fetcher.acquire(
            args.track_id,
            args.out,
            max_attempts=args.max_attempts,
            prefer_video_ids=args.prefer,
            exclude_video_ids=args.exclude,
            direct_url=args.url,
            keep_rejected=args.keep_rejected,
        )
        _print(result.to_json())
        return 0 if result.status in ("verified", "unverified") else 1

    if args.command == "bench":
        from .bench import run_bench

        report = run_bench(
            fetcher,
            args.set_path,
            args.out,
            label=args.label or _label_from(args.out),
            audio_dir=args.audio_dir,
            delay_s=max(args.delay, 5.0),
            only=args.only,
        )
        _print(report["summary"])
        return 0

    if args.command == "worker":
        from .worker import run_worker

        if not args.token:
            parser.error("нужен --token или FETCHER_WORKER_TOKEN")
        return run_worker(
            fetcher,
            args.server,
            args.token,
            worker_id=args.worker_id,
            once=args.once,
            poll_s=args.poll,
        )

    parser.error(f"неизвестная команда {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
