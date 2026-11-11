"""Скачивание файлов моделей с проверкой sha256 и докачкой.

essentia.upf.edu иногда рвёт соединение посреди файла, поэтому каждый файл качается с
повторами и докачкой по Range, а в папку попадает только после проверки суммы.
"""

from __future__ import annotations

import hashlib
import json
import logging
import time
from pathlib import Path

import httpx

from .manifest import MODELS, ModelFile, all_files

log = logging.getLogger("dalywave_ml.download")

LOCK_NAME = "manifest.lock.json"


class DownloadError(Exception):
    pass


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def _fetch(url: str, dest: Path, size: int, attempts: int = 6, timeout_s: float = 60.0) -> None:
    part = dest.with_name(dest.name + ".part")
    for attempt in range(1, attempts + 1):
        have = part.stat().st_size if part.exists() else 0
        headers = {"Range": f"bytes={have}-"} if have else {}
        try:
            with httpx.stream(
                "GET", url, headers=headers, follow_redirects=True, timeout=timeout_s
            ) as r:
                if r.status_code == 416 and size and have >= size:
                    break
                if r.status_code not in (200, 206):
                    raise DownloadError(f"{url}: HTTP {r.status_code}")
                mode = "ab" if r.status_code == 206 else "wb"
                with part.open(mode) as f:
                    for chunk in r.iter_bytes(1 << 20):
                        f.write(chunk)
            if not size or part.stat().st_size >= size:
                break
        except (httpx.HTTPError, DownloadError) as e:
            if attempt == attempts:
                raise DownloadError(f"не удалось скачать {url}: {e}") from e
            wait = min(60, 2**attempt)
            log.warning("скачивание прервалось (%s), повтор через %d с", e, wait)
            time.sleep(wait)
    part.replace(dest)


def _ensure(file: ModelFile, root: Path, lock: dict[str, dict[str, object]], force: bool) -> None:
    dest = root / file.path
    dest.parent.mkdir(parents=True, exist_ok=True)
    known = lock.get(file.path)
    if file.exported:
        if not dest.exists() or not known or known.get("sourceSha256") != file.sha256:
            log.warning("%s делает экспортёр: export/export_clap.py --out %s", file.path, root)
        return
    if (
        not force
        and dest.exists()
        and known
        and known.get("sourceSha256") == file.sha256
        and sha256_file(dest) == known.get("sha256")
    ):
        log.info("уже есть: %s", file.path)
        return
    # Файл, скачанный раньше без записи в lock, не качается заново, если сумма сходится.
    if force or not dest.exists() or sha256_file(dest) != file.sha256:
        log.info("качаю %s", file.url)
        _fetch(file.url, dest, file.size)
    actual = sha256_file(dest)
    if actual != file.sha256:
        dest.unlink(missing_ok=True)
        raise DownloadError(f"{file.path}: sha256 {actual}, ожидалась {file.sha256}")
    if file.size and dest.stat().st_size != file.size:
        raise DownloadError(f"{file.path}: размер {dest.stat().st_size}, ожидался {file.size}")
    lock[file.path] = {
        "url": file.url,
        "sourceSha256": file.sha256,
        "sha256": sha256_file(dest),
        "size": dest.stat().st_size,
        "fp16Storage": file.fp16_storage,
    }


def read_lock(root: Path) -> dict[str, dict[str, object]]:
    path = root / LOCK_NAME
    if not path.exists():
        return {}
    return json.loads(path.read_text())


def download(root: Path, *, only: set[str] | None = None, force: bool = False) -> None:
    root.mkdir(parents=True, exist_ok=True)
    lock = read_lock(root)
    for model in MODELS:
        if only and model.key.split(":")[0] not in only:
            continue
        for file in model.files:
            _ensure(file, root, lock, force)
            (root / LOCK_NAME).write_text(json.dumps(lock, indent=2, ensure_ascii=False) + "\n")


def verify(root: Path) -> list[str]:
    """Пустой список — все файлы на месте и совпадают с записанными суммами."""
    lock = read_lock(root)
    problems = []
    for file in all_files():
        dest = root / file.path
        known = lock.get(file.path)
        hint = " — его собирает export/export_clap.py" if file.exported else ""
        if not dest.exists():
            problems.append(f"нет файла {file.path}{hint}")
        elif not known or known.get("sourceSha256") != file.sha256:
            problems.append(f"{file.path}: нет записи о проверке для этой версии{hint}")
        elif sha256_file(dest) != known.get("sha256"):
            problems.append(f"{file.path}: файл изменён после скачивания")
    return problems


def main() -> None:
    """`python -m dalywave_ml.download <папка>` — так модели качаются при сборке образа."""
    import sys

    logging.basicConfig(level=logging.INFO, format="%(message)s")
    # httpx на INFO печатает каждую подписанную ссылку CDN — в логе сборки это шум.
    logging.getLogger("httpx").setLevel(logging.WARNING)
    root = Path(sys.argv[1] if len(sys.argv) > 1 else "models")
    download(root)
    problems = verify(root)
    if problems:
        raise SystemExit("\n".join(problems))


if __name__ == "__main__":
    main()
