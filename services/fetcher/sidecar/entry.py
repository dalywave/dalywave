"""Точка входа собранного загрузчика (PyInstaller): та же командная строка, что `fetcher`."""

import sys

from dalywave_fetcher.cli import main

if __name__ == "__main__":
    sys.exit(main())
