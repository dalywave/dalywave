from __future__ import annotations

import pytest
from helpers import make_track

from dalywave_fetcher.deezer import DeezerTrack


@pytest.fixture
def track() -> DeezerTrack:
    return make_track()
