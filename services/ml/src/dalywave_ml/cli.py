"""Командная строка: `ml serve | analyze | embed-text | models | bench`."""

from __future__ import annotations

import argparse
import dataclasses
import json
import logging
import statistics
import sys
import time
from pathlib import Path
from typing import Any

from .config import Settings


def _settings(args: argparse.Namespace) -> Settings:
    settings = Settings.from_env()
    overrides: dict[str, Any] = {}
    if getattr(args, "models", None):
        overrides["models_dir"] = Path(args.models)
    if getattr(args, "threads", None):
        overrides["threads"] = args.threads
    return dataclasses.replace(settings, **overrides) if overrides else settings


def cmd_serve(args: argparse.Namespace) -> None:
    import uvicorn

    from .download import ensure_start_models
    from .server import create_app

    settings = _settings(args)
    # Модели MTG — до HTTP: пока они качаются, проверка здоровья не зелёная, а сбой скачивания
    # завершает процесс, и docker перезапускает его (restart: unless-stopped) — новая попытка
    ensure_start_models(settings.mtg_dir)
    uvicorn.run(create_app(settings), host=args.host, port=args.port, log_level="info")


def _load_audio(source: str, settings: Settings):
    import tempfile

    from .audio import decode
    from .server import fetch

    if source.startswith(("http://", "https://")):
        with tempfile.TemporaryDirectory(prefix="ml-") as tmp:
            dest = Path(tmp) / "input"
            fetch(source, dest, settings)
            return decode(dest, max_seconds=settings.max_duration_s)
    return decode(Path(source), max_seconds=settings.max_duration_s)


def cmd_analyze(args: argparse.Namespace) -> None:
    from .analysis import Analyzer

    settings = _settings(args)
    analyzer = Analyzer(settings)
    result = analyzer.analyze(_load_audio(args.source, settings), args.mode)
    if not args.vectors:
        result["sound"] = f"<{len(result['sound'])} символов base64>"
        result["semantic"] = f"<{len(result['semantic'])} символов base64>"
    print(json.dumps(result, ensure_ascii=False, indent=2))


def cmd_embed_text(args: argparse.Namespace) -> None:
    from .analysis import Analyzer

    print(json.dumps(Analyzer(_settings(args)).embed_text(args.text), ensure_ascii=False, indent=2))


def cmd_models(args: argparse.Namespace) -> None:
    from .download import download, verify, write_attribution
    from .manifest import image_files, start_files

    settings = Settings.from_env()
    # --dest — всё в одну папку; иначе модели MTG — в ML_MTG_MODELS_DIR, если она задана
    dest = Path(args.dest) if args.dest else settings.models_dir
    mtg = Path(args.dest) if args.dest else settings.mtg_dir
    if args.action == "download":
        only = set(args.only.split(",")) if args.only else None
        download(dest, only=only, force=args.force, on_start=False)
        download(mtg, only=only, force=args.force, on_start=True)
        write_attribution(mtg)
    problems = verify(dest, image_files()) + verify(mtg, start_files())
    for p in problems:
        print(p, file=sys.stderr)
    if problems:
        sys.exit(1)
    print(f"модели в порядке: {dest}" + (f", MTG — {mtg}" if mtg != dest else ""))


def cmd_bench(args: argparse.Namespace) -> None:
    """Время анализа: модели загружаются один раз, каждый файл — `--repeat` прогонов."""
    from .analysis import Analyzer
    from .audio import decode

    settings = _settings(args)
    started = time.perf_counter()
    analyzer = Analyzer(settings)
    load_s = time.perf_counter() - started
    rows = []
    for source in args.files:
        for mode in args.modes.split(","):
            totals, stages = [], []
            for _ in range(args.repeat):
                t0 = time.perf_counter()
                audio = decode(Path(source), max_seconds=settings.max_duration_s)
                t1 = time.perf_counter()
                result = analyzer.analyze(audio, mode)
                t2 = time.perf_counter()
                totals.append(t2 - t0)
                stages.append({"decode": round((t1 - t0) * 1000), **result["timingsMs"]})
            best = min(range(len(totals)), key=lambda i: totals[i])
            rows.append(
                {
                    "file": Path(source).name,
                    "mode": mode,
                    "audioS": round(audio.duration_s, 1),
                    "medianS": round(statistics.median(totals), 3),
                    "minS": round(min(totals), 3),
                    "stagesMs": stages[best],
                }
            )
            print(json.dumps(rows[-1], ensure_ascii=False), flush=True)
    summary = {"threads": settings.threads, "modelLoadS": round(load_s, 2), "runs": rows}
    if args.out:
        Path(args.out).write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")


def main() -> None:
    logging.basicConfig(
        level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s: %(message)s"
    )
    logging.getLogger("httpx").setLevel(logging.WARNING)
    parser = argparse.ArgumentParser(prog="ml", description="DalyWave: анализ звучания")
    sub = parser.add_subparsers(dest="cmd", required=True)

    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--models", help="папка моделей (по умолчанию ML_MODELS_DIR или ./models)")
    common.add_argument("--threads", type=int, help="потоки onnxruntime (по умолчанию — все ядра)")

    p = sub.add_parser("serve", parents=[common], help="HTTP API")
    p.add_argument("--host", default="0.0.0.0")
    p.add_argument("--port", type=int, default=8403)
    p.set_defaults(func=cmd_serve)

    p = sub.add_parser("analyze", parents=[common], help="разобрать файл или ссылку")
    p.add_argument("source")
    p.add_argument("--mode", choices=["preview", "full"], default="full")
    p.add_argument("--vectors", action="store_true", help="печатать векторы целиком")
    p.set_defaults(func=cmd_analyze)

    p = sub.add_parser("embed-text", parents=[common], help="вектор CLAP для текста")
    p.add_argument("text")
    p.set_defaults(func=cmd_embed_text)

    p = sub.add_parser("models", help="скачать или проверить файлы моделей")
    p.add_argument("action", choices=["download", "verify"])
    p.add_argument(
        "--dest", help="папка (по умолчанию ML_MODELS_DIR или ./models, MTG — ML_MTG_MODELS_DIR)"
    )
    p.add_argument("--only", help="через запятую: effnet,head,clap,beat_this,skey")
    p.add_argument("--force", action="store_true", help="скачать заново, даже если файлы есть")
    p.set_defaults(func=cmd_models)

    p = sub.add_parser("bench", parents=[common], help="замер скорости")
    p.add_argument("files", nargs="+")
    p.add_argument("--modes", default="preview,full")
    p.add_argument("--repeat", type=int, default=3)
    p.add_argument("--out")
    p.set_defaults(func=cmd_bench)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
