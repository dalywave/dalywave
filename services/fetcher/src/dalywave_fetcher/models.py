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


class SelftestRequest(_Model):
    deezer_track_id: int | None = None


class Job(AcquireRequest):
    """Задание, которое загрузчик забирает с сервера (`POST /api/fetcher/jobs/claim`)."""

    id: str
    kind: Literal["acquire"] = "acquire"
    lease_expires_at: datetime | None = None
