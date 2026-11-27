# Install DalyWave on your server

In Ukrainian: [install.uk.md](install.uk.md)

DalyWave runs as a few containers from prebuilt images (linux/amd64 and linux/arm64):

| Container | Image | What it does |
|---|---|---|
| `api` | `ghcr.io/dalywave/server` | API, web interface, file links for players |
| `worker` | `ghcr.io/dalywave/server` | background jobs: music folder scans, covers, loudness, analysis queue |
| `ml` | `ghcr.io/dalywave/ml` | sound analysis for the wave, radio, DJ transitions and search by sound |
| `db` | `pgvector/pgvector:pg18` | PostgreSQL with pgvector |
| `backup` | `postgres:18-alpine` | daily database dump, the last 14 kept |

The Deezer catalog and the YouTube downloader are off by default (see [Optional modules](#optional-modules)).

## Requirements

- Linux with Docker Engine and Docker Compose v2. A Raspberry Pi 5 or an ARM cloud VM works too.
- Memory: about 1 GB for the server and the database, plus 1.5–3 GB for `ml`. With less than 4 GB
  in total, run without `ml` (`ML_REPLICAS=0`).
- Disk: a few GB for the images, plus the database and server files: covers, clips and copies of
  tracks for slow networks and for formats browsers can't play (at most `DALYWAVE_RENDITIONS_MAX_GB`,
  20 GB by default). Your music is never copied.
- A music folder whose files are readable by uid 10001 (world-readable is enough).
- Internet access on the first start: the images come from ghcr.io, and `ml` downloads its MTG models
  (23 MB) from essentia.upf.edu — see [Sound analysis models](#sound-analysis-models).
- For the iPhone, iPad, Apple TV and desktop apps — HTTPS: they accept plain HTTP only for
  `localhost`. See [Reverse proxy and HTTPS](#reverse-proxy-and-https).

## Quick start

1. Download the compose file and the example settings:

   ```sh
   mkdir dalywave && cd dalywave
   curl -fsSLO https://raw.githubusercontent.com/dalywave/dalywave/main/infra/selfhost/docker-compose.yml
   curl -fsSL -o .env https://raw.githubusercontent.com/dalywave/dalywave/main/infra/selfhost/.env.example
   chmod 600 .env
   ```

2. Generate the secrets into `.env` (on macOS write `sed -i ''`):

   ```sh
   sed -i "s/^DALYWAVE_SECRET_KEY=.*/DALYWAVE_SECRET_KEY=$(openssl rand -hex 32)/" .env
   sed -i "s/^POSTGRES_PASSWORD=.*/POSTGRES_PASSWORD=$(openssl rand -hex 24)/" .env
   ```

3. Edit `.env`:
   - `DALYWAVE_PUBLIC_URL` — the exact address people will open, e.g. `http://192.168.1.20:8400`
     or `https://music.example.com`. Sign-in breaks if it differs from what the browser shows.
   - `MUSIC_DIR` — your music folder on this machine, e.g. `/srv/music`.

4. Start it and check the server:

   ```sh
   docker compose up -d
   curl -fsS http://localhost:8400/api/healthz   # {"status":"ok",…}
   ```

   The first start takes longer: besides pulling the images, `ml` downloads its models (a minute or
   a few — essentia.upf.edu is slow at times) and stays unhealthy until then. Music already plays;
   analysis catches up once `ml` is ready (`docker compose logs -f ml`).

5. Open `DALYWAVE_PUBLIC_URL` right away and create the administrator on the "Welcome" screen. Until
   then this screen is open to anyone who reaches the server.

6. The worker scans the music folder on start, every 6 hours and on changes. Progress, skipped files
   and the "Rescan" button are in **Admin → Library**. Invite other people in **Admin → Invites &
   sign-in**.

## Reverse proxy and HTTPS

The compose file publishes one plain HTTP port and has no TLS. Put any reverse proxy in front:

- in `.env`: `HTTP_PORT=127.0.0.1:8400` (only the proxy can reach it) and
  `DALYWAVE_PUBLIC_URL=https://music.example.com`, then `docker compose up -d`;
- the proxy must pass WebSocket upgrades (`/api/ws`), allow request bodies up to 1 GB (uploads),
  not buffer responses (audio is streamed with Range requests) and set `X-Forwarded-For` to the
  client address (the server uses it for rate limits).

Caddy gets a certificate and does all of the above by itself:

```
music.example.com {
    reverse_proxy 127.0.0.1:8400
}
```

nginx:

```nginx
map $http_upgrade $connection_upgrade { default upgrade; '' close; }

server {
    listen 443 ssl;
    server_name music.example.com;
    # ssl_certificate …

    location / {
        proxy_pass http://127.0.0.1:8400;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
        proxy_buffering off;
        proxy_read_timeout 1h;
        client_max_body_size 1g;
    }
}
```

A proxy that runs in Docker can join the `dalywave_default` network and point at `http://api:8400`.

## Sign-in

Local accounts and the first-run screen are on by default (`DALYWAVE_LOCAL_AUTH=true`). Any OpenID
Connect provider — Authentik, Keycloak, Pocket ID, Google and others — is added with
`DALYWAVE_OIDC_<ID>_*` variables in `.env`; register the redirect URI
`<DALYWAVE_PUBLIC_URL>/api/auth/oidc/<id>/callback` at the provider. Roles, invites and examples for
each provider: [auth.md](auth.md).

## Optional modules

All of them are set in `.env`, followed by `docker compose up -d`:

- **Deezer catalog**: `DALYWAVE_DEEZER=true` — search the whole catalog, charts, new releases, similar
  artists; your files get matched to it.
- **Downloader**: downloading from YouTube may break its terms of service — enable it only for
  yourself and knowing that. `COMPOSE_PROFILES=downloader`, `DALYWAVE_DOWNLOADER=true` and
  `DALYWAVE_FETCHER_MODE=remote,server` start the `fetcher` and `pot` containers.
- **Without sound analysis**: `ML_REPLICAS=0`. Music plays, but the wave shuffles your library,
  DJ transitions and search by sound are unavailable, and **Admin → Health** shows `ml` as down.

### Sound analysis models

`ml` relies on Discogs-EffNet and its classifier heads by the Music Technology Group (Universitat
Pompeu Fabra), licensed **CC BY-NC-SA 4.0 — non-commercial use only**. They are not part of the image:
on its first start the container downloads them unmodified from essentia.upf.edu (20 files, 23 MB)
into the `ml-models` volume, checks their sha256 and writes `ATTRIBUTION.md` next to them; later
starts use the volume. If the download fails, the container exits and Docker restarts it to try
again. For commercial use you need a license from MTG — or run without `ml` (`ML_REPLICAS=0`).
The other models and programs in the image and their licenses: `/usr/share/doc/dalywave-ml/`.

How the music folder is read (tags, covers, formats, renames): [music.md](music.md). Storing server
files in S3 (Cloudflare R2, MinIO) instead of the `data` volume: [storage.md](storage.md).

## Updating

```sh
docker compose pull
docker compose up -d
```

The server migrates the database on start; migrations only go forward, so make a backup first and
don't go back to an older image afterwards. `IMAGE_TAG=latest` follows releases; pin a version
such as `IMAGE_TAG=0.1.19` to update only when you change it. Read the release notes before
updating, and re-download `docker-compose.yml` when they say so.

## Backups

**Database.** The `backup` container writes `dalywave-YYYY-MM-DD.sql.gz` every day to the `backups`
volume and keeps the last 14; **Admin → Backups** lists and checks them. Copy them off the machine
regularly:

```sh
docker compose cp backup:/backup ./backups
```

**Server files** (covers, copies, clips, imports) are in the `data` volume:

```sh
docker run --rm -v dalywave_data:/data:ro -v "$PWD":/out alpine tar czf /out/dalywave-data.tgz -C /data .
```

Your music folder and `.env` are yours to back up as usual; without the same `DALYWAVE_SECRET_KEY`
everyone has to sign in again.

**Restore** into a fresh installation (same `.env`):

```sh
docker compose up -d db
docker compose exec -T db dropdb -U dalywave dalywave
docker compose exec -T db createdb -U dalywave dalywave
gunzip -c dalywave-2026-11-23.sql.gz | docker compose exec -T db psql -q -U dalywave -d dalywave
docker run --rm -v dalywave_data:/data -v "$PWD":/in alpine tar xzf /in/dalywave-data.tgz -C /data
docker compose up -d
```

## Troubleshooting

- `docker compose ps` — all containers should be `running` (`api` — `healthy`).
- `docker compose logs -f api worker` — set `DALYWAVE_LOG_FORMAT=pretty` for readable logs.
- Sign-in fails right after entering a password: `DALYWAVE_PUBLIC_URL` doesn't match the address in
  the browser (scheme, host or port).
- The library is empty: check `MUSIC_DIR` and file permissions; skipped files and the reason are in
  **Admin → Library**.
