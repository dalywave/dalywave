"""Формы запросов HTTP API и заданий «домашнего загрузчика». Поля JSON — camelCase."""

from __future__ import annotations

from datetime import datetime
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field
from pydantic.alias_generators import to_camel

from .acquire import UploadTarget


class _Model(BaseModel):
    model_config = ConfigDict(alias_generator=to_camel, populate_by_name=True, extra="ignore")


class UploadSpec(_Model):
    """Куда положить файл: подписанная ссылка R2 (S3). Content-Type в подпись не входит —
    контейнер (ogg или mp4) становится известен только после скачивания."""

    url: str
    method: Literal["PUT", "POST"] = "PUT"
    headers: dict[str, str] = Field(default_factory=dict)
    expires_at: datetime | None = None

    def target(self) -> UploadTarget:
        return UploadTarget(url=self.url, method=self.method, headers=dict(self.headers))


class MatchRequest(_Model):
    deezer_track_id: int
    videos: Literal["auto", "always", "never"] = "auto"


class AcquireRequest(_Model):
    deezer_track_id: int
    upload: UploadSpec | None = None
    max_attempts: int = Field(default=3, ge=1, le=6)
    # Видео, выбранные вручную в «Спорных», пробуются первыми; исключённые — никогда.
    prefer_video_ids: list[str] = Field(default_factory=list)
    exclude_video_ids: list[str] = Field(default_factory=list)
    # Ссылка, вставленная в «Спорных» (SoundCloud, Bandcamp…): качается она, а не находка поиска.
    direct_url: str | None = None


class SelftestRequest(_Model):
    deezer_track_id: int | None = None


class YtPlaylistRequest(_Model):
    """Перенос: треки публичного плейлиста YouTube / YouTube Music."""

    playlist_id: str = Field(min_length=2, max_length=80, pattern=r"^[A-Za-z0-9_-]+$")


class YtSongsRequest(_Model):
    """Перенос: имена видео по id (архив Google Takeout хранит только id)."""

    video_ids: list[str] = Field(min_length=1, max_length=50)


class Job(AcquireRequest):
    """Задание, которое загрузчик забирает с сервера (`POST /api/fetcher/jobs/claim`)."""

    id: str
    kind: Literal["acquire"] = "acquire"
    lease_expires_at: datetime | None = None


class ClipJob(_Model):
    """Задание «клип» (`POST /api/fetcher/clips/claim`): найти официальный клип трека, вырезать
    отрывок вокруг припева и положить MP4 по подписанной ссылке."""

    id: str
    track_id: str
    title: str
    artists: list[str] = Field(default_factory=list)
    duration_ms: int = Field(gt=0)
    hook_start_ms: int = Field(default=0, ge=0)
    length_ms: int = Field(default=12_000, ge=3_000, le=30_000)
    isrc: str | None = None
    exclude_video_ids: list[str] = Field(default_factory=list)
    # Лейбл альбома (Deezer): его канал тоже официальный
    label: str | None = None
    # auto — найти и нарезать; pinned — нарезать video_id (выбор администратора); search — только
    # кандидаты; review — перепроверить нарезанный current_video_id новым отбором
    mode: Literal["auto", "pinned", "search", "review"] = "auto"
    video_id: str | None = None
    current_video_id: str | None = None
    upload: UploadSpec
    lease_expires_at: datetime | None = None
