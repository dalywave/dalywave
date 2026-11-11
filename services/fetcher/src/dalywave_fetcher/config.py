"""Настройки сервиса из переменных окружения."""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from pathlib import Path


def _env(name: str) -> str | None:
    value = os.environ.get(name, "").strip()
    return value or None


def _list_env(name: str) -> tuple[str, ...] | None:
    value = _env(name)
    if value is None:
        return None
    return tuple(c.strip() for c in value.split(",") if c.strip())


def _float_env(name: str, default: float) -> float:
    value = _env(name)
    return float(value) if value else default


@dataclass(frozen=True)
class Settings:
    # Адрес сервиса PO-токенов bgutil (http://pot:4416). Пусто — токены не запрашиваются.
    pot_url: str | None = None
    # Файл cookies в формате Netscape. Копируется в рабочую папку: yt-dlp перезаписывает его.
    cookies_file: Path | None = None
    # Клиенты плеера YouTube через запятую (mweb, tv, web_safari…). Пусто — выбор yt-dlp.
    player_clients: tuple[str, ...] = ()
    # Клиенты для повтора после 403. None — выбрать самому, () — не повторять.
    fallback_clients: tuple[str, ...] | None = None
    data_dir: Path = field(default_factory=lambda: Path("data"))
    deno_path: str | None = None
    proxy: str | None = None
    # Адрес, с которого yt-dlp ходит в YouTube: 0.0.0.0 — только IPv4, :: — только IPv6.
    source_address: str | None = None
    # Пауза между скачиваниями внутри одного процесса, чтобы не злить YouTube.
    min_download_interval_s: float = 3.0
    selftest_track_id: int = 908604612

    @classmethod
    def from_env(cls) -> Settings:
        clients = _list_env("FETCHER_PLAYER_CLIENTS") or ()
        fallback = _list_env("FETCHER_FALLBACK_CLIENTS")
        cookies = _env("FETCHER_COOKIES")
        return cls(
            pot_url=_env("FETCHER_POT_URL"),
            cookies_file=Path(cookies) if cookies else None,
            player_clients=clients,
            fallback_clients=() if fallback == ("none",) else fallback,
            data_dir=Path(_env("FETCHER_DATA_DIR") or "data"),
            deno_path=_env("FETCHER_DENO"),
            proxy=_env("FETCHER_PROXY"),
            source_address=_env("FETCHER_SOURCE_ADDRESS"),
            min_download_interval_s=_float_env("FETCHER_MIN_INTERVAL_S", 3.0),
            selftest_track_id=int(_env("FETCHER_SELFTEST_TRACK") or 908604612),
        )

    def describe(self) -> dict[str, object]:
        """Настройки без секретов — для отчётов и /v1/healthz."""
        return {
            "potUrl": self.pot_url,
            "cookies": self.cookies_file is not None,
            "playerClients": list(self.player_clients) or None,
            "fallbackClients": None
            if self.fallback_clients is None
            else list(self.fallback_clients),
            "proxy": self.proxy is not None,
            "sourceAddress": self.source_address,
            "dataDir": str(self.data_dir),
        }
