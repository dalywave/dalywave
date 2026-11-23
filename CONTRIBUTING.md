# Contributing to DalyWave

Thanks for taking the time to help. Bug reports, fixes and documentation improvements are welcome.
For a new feature or a larger change, please open an issue first so we can agree on the approach
before you spend time on code.

Security problems are not reported in public issues: see [SECURITY.md](SECURITY.md).

## Requirements

- Docker (Postgres and S3-compatible storage for development)
- Rust — the version pinned in [`rust-toolchain.toml`](rust-toolchain.toml), with `rustfmt` and `clippy`
- [Bun](https://bun.sh) for the interface (CI uses 1.2.21)
- [uv](https://docs.astral.sh/uv/) for the Python services (Python 3.13)
- `ffmpeg` and `fpcalc` (Chromaprint) on `PATH` if you run the job worker against real audio files

For the apps you also need the Tauri CLI (`cargo install tauri-cli --version 2.12.4 --locked`, the
version CI uses) and, for the Apple platforms, Xcode and `xcodegen` (`brew install xcodegen`).

## Repository layout

```
apps/ui             Nuxt interface: the website, the admin panel and the screens of every app
apps/shell          Tauri shell: macOS, Windows, iPhone and iPad; Xcode project with Apple TV,
                    Apple Watch, widgets and extensions (gen/apple)
apps/streamdeck     Stream Deck plugin
crates/server       Rust server: API, sign-in, library, recommendations, job worker
crates/protocol     API and WebSocket types shared by the server and the apps
crates/client       native client core: audio engine, offline downloads, local SQLite store, sync
crates/taste        taste model: training, inference, ONNX export
crates/transitions  DJ transitions: phrases, mix points, tempo map, key compatibility, set order
crates/textmatch    fuzzy title matching for search (wrong keyboard layout, transliteration, typos)
services/ml         audio analysis service (Python, ONNX Runtime on CPU)
services/fetcher    optional downloader (Python, yt-dlp)
design              mockups, brand assets and data for them
infra               Dockerfile and docker compose files
tools               screenshot tool and helper scripts
docs                plan, conventions, decisions, data model, self-hosting guides
```

## Local development

```sh
make up        # Postgres 18 with pgvector and S3 (SeaweedFS) in Docker
make api       # API on http://localhost:8400
make worker    # job worker: scans, covers, subscriptions, audio analysis
make ui        # interface on http://localhost:3400 (proxies /api to :8400)
make design    # mockups on http://localhost:4400
make test      # all tests
make lint      # style checks
```

`make help` lists every command. A few more you will need:

```sh
make db-reset      # recreate the local database (data is lost)
make ml            # audio analysis service on 127.0.0.1:8403; build the image first:
                   #   docker build -t dalywave-ml:0.1.0 services/ml
make api-types     # regenerate the interface's TypeScript types from the server's OpenAPI
make sqlx-prepare  # refresh the offline sqlx query data in .sqlx/ after changing SQL
make fmt           # format Rust, the interface and the Python services
```

Settings are `DALYWAVE_*` environment variables. Every variable is listed with a comment in
[`.env.example`](.env.example) (the comments are in Russian); copy it to `.env` in the repository root
and adjust. `.env` is never committed. The self-hosting guides describe the main settings in detail:
[sign-in](docs/self-hosting/auth.md), [music folders](docs/self-hosting/music.md),
[storage](docs/self-hosting/storage.md).

Database migrations live in `crates/server/migrations/` and are applied when the server starts.
They only move forward: add a new migration instead of editing an old one.

### Tests

`make test` runs the Rust tests against a separate Postgres container for this checkout
(`tools/test-pg.sh`), then type-checks the interface and runs the Python tests. External services
(Deezer, the downloader, OIDC providers) are replaced with local stubs, so the tests do not need the
network.

### Apps

- Desktop (macOS, Windows): [apps/shell/README.md](apps/shell/README.md). `make app` builds the macOS
  app and `.dmg`; the Windows installer is built by the [desktop workflow](.github/workflows/desktop.yml).
- iPhone, iPad, Apple TV and Apple Watch: [apps/shell/IOS.md](apps/shell/IOS.md). `make ios` regenerates
  the Xcode project and opens it.

These documents are currently in Russian.

## Conventions

Read [docs/conventions.md](docs/conventions.md) before your first change (in Russian). It covers
ports, environment variables, Rust and API style, the interface stack and the screenshot check every
screen goes through. In short:

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `bun run lint` and `ruff` pass without
  warnings.
- No `unwrap`, `expect` or `panic!` in request handlers.
- API types come from OpenAPI (`make api-types`); they are not duplicated by hand.
- Interface strings go through i18n, in both `ru` and `en`.
- For interface changes, take screenshots with `bun tools/shot.mjs` at phone and desktop sizes and look
  at them before opening the pull request.

## Pull requests

- Keep a pull request to one change. Describe what it changes and why.
- Run `make lint` and `make test` before pushing.
- Include screenshots for visible interface changes.
- Commit messages: a short summary line, then a paragraph on what changed and why.

By submitting a pull request you agree that your contribution is licensed under the
[GNU Affero General Public License v3.0](LICENSE), the license of this project.
