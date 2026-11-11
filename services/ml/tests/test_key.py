import numpy as np
import pytest

from dalywave_ml.key import camelot, undo_outer_softmax

# Колесо Камелота: мажоры — B, миноры — A; соседние числа — квинты.
WHEEL = {
    "C Major": "8B",
    "G Major": "9B",
    "D Major": "10B",
    "A Major": "11B",
    "E Major": "12B",
    "B Major": "1B",
    "F# Major": "2B",
    "C# Major": "3B",
    "G# Major": "4B",
    "D# Major": "5B",
    "Bb Major": "6B",
    "F Major": "7B",
    "A minor": "8A",
    "E minor": "9A",
    "B minor": "10A",
    "F# minor": "11A",
    "C# minor": "12A",
    "G# minor": "1A",
    "D# minor": "2A",
    "Bb minor": "3A",
    "F minor": "4A",
    "C minor": "5A",
    "G minor": "6A",
    "D minor": "7A",
}


@pytest.mark.parametrize(("name", "code"), WHEEL.items())
def test_camelot(name, code):
    assert camelot(name) == code


def test_enharmonic_spellings():
    assert camelot("Db Major") == camelot("C# Major") == "3B"
    assert camelot("Eb minor") == camelot("D# minor") == "2A"
    assert camelot("A# minor") == "3A"


def test_relative_keys_share_number():
    for major, minor in [("C Major", "A minor"), ("Bb Major", "G minor"), ("E Major", "C# minor")]:
        assert camelot(major)[:-1] == camelot(minor)[:-1]


def test_undo_outer_softmax_recovers_probabilities():
    rng = np.random.default_rng(1)
    p = rng.dirichlet(np.ones(24) * 0.3)
    q = np.exp(p) / np.exp(p).sum()  # то, что отдаёт граф S-KEY
    np.testing.assert_allclose(undo_outer_softmax(q.astype(np.float32)), p, atol=1e-5)
    # Второй softmax прижимает всё к 1/24 — победитель почти не отличим от остальных.
    assert q.max() < 0.1 < p.max()
