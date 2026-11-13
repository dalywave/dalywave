.DEFAULT_GOAL := help
SHELL := /bin/bash
COMPOSE_LOCAL := docker compose -f infra/docker-compose.local.yml
export DALYWAVE_DATABASE_URL ?= postgres://dalywave:dalywave@localhost:5400/dalywave
export DATABASE_URL ?= $(DALYWAVE_DATABASE_URL)

help: ## показать команды
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

up: ## поднять Postgres и S3 локально
	$(COMPOSE_LOCAL) up -d --wait

down: ## остановить локальные сервисы
	$(COMPOSE_LOCAL) down

db-reset: ## пересоздать локальную базу (данные пропадут)
	$(COMPOSE_LOCAL) down -v postgres && $(COMPOSE_LOCAL) up -d --wait postgres

api: ## запустить API на :8400
	cargo run -p dalywave-server --bin dalywave-api

worker: ## исполнитель очереди задач (скачивание, обложки, подписки, анализ звука, разведчик)
	cargo run -p dalywave-server --bin dalywave-worker

# Сервис анализа звука в docker. Образ: docker build -t dalywave-ml:0.1.0 services/ml (~10 минут в первый раз).
# Файлы скачанного он берёт из SeaweedFS на этом Mac: DALYWAVE_S3_ML_ENDPOINT=http://host.docker.internal:8401 в .env
ML_IMAGE ?= dalywave-ml:0.1.0
ml: ## сервис анализа звука на 127.0.0.1:8403 (docker)
	@docker rm -f dalywave-ml-dev >/dev/null 2>&1 || true
	docker run -d --name dalywave-ml-dev --cpus 6 -p 127.0.0.1:8403:8403 $(ML_IMAGE)

# Аудио качает этот Mac: YouTube не отдаёт его серверу. Токен dwk_… — из админки («Источники»):
# TOKEN=… или файл ~/.config/dalywave/downloader-token. В автозапуск: services/fetcher/launchd/install.sh
SERVER ?= https://wave.daly.rest
downloader: ## загрузчик на этом Mac: make downloader [SERVER=http://localhost:8400] [TOKEN=dwk_…]
	@FETCHER_WORKER_TOKEN="$(TOKEN)" bash services/fetcher/launchd/run.sh "$(SERVER)"

ui: ## интерфейс на :3400 (проксирует /api на :8400)
	cd apps/ui && bun run dev

# Загрузчик внутри приложения (services/fetcher/SIDECAR.md) → services/fetcher/dist/sidecar.
# Собирается, если его нет или исходники fetcher новее сборки (MANIFEST.json пишется последним);
# CLEAN=1 — всё заново. Сборка для Windows — sidecar/build-windows.ps1 на Windows (CI desktop.yml)
SIDECAR_STAMP := services/fetcher/dist/sidecar/MANIFEST.json
sidecar: ## загрузчик для приложения: собрать, если его нет или он старше исходников fetcher
	@if [ "$${CLEAN:-}" = 1 ] || [ ! -f $(SIDECAR_STAMP) ] || [ -n "$$(find services/fetcher/src \
	    services/fetcher/sidecar services/fetcher/pyproject.toml services/fetcher/uv.lock -type f \
	    -newer $(SIDECAR_STAMP) -not -path '*/__pycache__/*' -not -name '.DS_Store' -not -name '*.ps1' \
	    -print -quit)" ]; then \
	  bash services/fetcher/sidecar/build.sh; \
	else \
	  echo "Загрузчик свежий: services/fetcher/dist/sidecar"; \
	fi

# Подпись: Developer ID из связки ключей (иначе ad-hoc), ключ обновлений — вне репозитория,
# пароль к нему — в связке ключей (apps/shell/README.md). Программы загрузчика в Contents/MacOS
# Tauri подписывает тем же ключом с apps/shell/entitlements.plist. CI=true: DMG без AppleScript
# к Finder. Свои ключи сборки: make app TAURI_ARGS='--debug --bundles app'
UPDATER_KEY ?= $(HOME)/.tauri/dalywave-updater.key
app: sidecar ## приложение macOS с загрузчиком: DalyWave.app и .dmg в apps/shell/target/release/bundle
	@cd apps/shell && \
	id="$${APPLE_SIGNING_IDENTITY:-$$(security find-identity -v -p codesigning | sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -n1)}"; \
	extra=(); \
	if [ -f "$(UPDATER_KEY)" ]; then \
	  export TAURI_SIGNING_PRIVATE_KEY="$(UPDATER_KEY)"; \
	  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$$(security find-generic-password -s dalywave-updater-key -a "$$USER" -w)"; \
	else \
	  echo "Нет $(UPDATER_KEY) — сборка без файлов автообновления"; \
	  extra=(--config '{"bundle":{"createUpdaterArtifacts":false}}'); \
	fi; \
	echo "Подпись: $${id:-ad-hoc}"; \
	CI=true APPLE_SIGNING_IDENTITY="$${id:--}" cargo tauri build "$${extra[@]}" $(TAURI_ARGS)

# Выпуск приложения (apps/shell/README.md, «Выпуск»): версия — одна, в apps/shell/Cargo.toml.
# release-mac собирает с подписью обновлений, кладёт архив в R2 по ссылке от сервера и регистрирует
# выпуск. Токен выпусков — DALYWAVE_RELEASE_TOKEN или связка ключей (запись dalywave-release-token)
bump-version: ## новая версия приложения: make bump-version [VERSION=0.2.0], без VERSION — +1 к последней цифре
	@bash apps/shell/scripts/bump-version.sh "$(VERSION)"

release-mac: ## собрать и выпустить приложение macOS на сервер обновлений: make release-mac [NOTES="…"]
	@[ -f "$(UPDATER_KEY)" ] || { echo "Нет $(UPDATER_KEY): без ключа подписи обновлений выпуск невозможен"; exit 1; }
	@[ -n "$${DALYWAVE_RELEASE_TOKEN:-}" ] || security find-generic-password -s dalywave-release-token -a "$$USER" >/dev/null 2>&1 || \
	  { echo "Нет токена выпусков: DALYWAVE_RELEASE_TOKEN или запись dalywave-release-token в связке ключей"; exit 1; }
	@$(MAKE) --no-print-directory app
	@DALYWAVE_RELEASE_TOKEN="$${DALYWAVE_RELEASE_TOKEN:-$$(security find-generic-password -s dalywave-release-token -a "$$USER" -w)}" \
	  RELEASE_NOTES="$(NOTES)" bun apps/shell/scripts/release.ts --server "$(SERVER)"

# iPhone и iPad: Xcode-проект apps/shell/gen/apple (xcodegen), Rust собирает его фаза «Build Rust Code».
# Установка на телефон — Run в Xcode (apps/shell/IOS.md). Проверка звука без входа и без звука:
#   make ios-sim IOS_SIM='iPhone 16 (iOS 27)' — играет тестовый сервер tools/serve.py на :8420
IOS_PROJECT := apps/shell/gen/apple/dalywave-shell.xcodeproj
IOS_SIM ?= iPhone 16 (iOS 27)
ios: ## iPhone и iPad: пересобрать Xcode-проект и открыть его в Xcode
	@bash apps/shell/scripts/xcodegen.sh
	open $(IOS_PROJECT)

ios-build: ## iPhone: собрать release для устройства (подпись Apple Development, команда Y5KR5JPCRG)
	@bash apps/shell/scripts/xcodegen.sh
	set -o pipefail; xcodebuild -project $(IOS_PROJECT) -scheme dalywave-shell_iOS -configuration release \
	  -destination generic/platform=iOS -allowProvisioningUpdates build | tail -n 5

testflight: ## iPhone: собрать для App Store и загрузить в TestFlight (ключ API — scripts/testflight.sh)
	@bash apps/shell/scripts/testflight.sh

testflight-tv: ## Apple TV: та же запись DalyWave, платформа tvOS (EXPORT=local — без загрузки)
	@bash apps/shell/scripts/testflight.sh tv

ios-sim: ## симулятор: собрать, поставить и запустить с тестовым звуком на громкости 0
	@bash apps/shell/scripts/xcodegen.sh
	set -o pipefail; xcodebuild -project $(IOS_PROJECT) -scheme dalywave-shell_iOS -configuration release \
	  -destination 'platform=iOS Simulator,name=$(IOS_SIM)' -derivedDataPath apps/shell/target/xcode build | tail -n 3
	xcrun simctl boot '$(IOS_SIM)' 2>/dev/null || true
	xcrun simctl install '$(IOS_SIM)' apps/shell/target/xcode/Build/Products/release-iphonesimulator/DalyWave.app
	SIMCTL_CHILD_DW_VOLUME=0 SIMCTL_CHILD_DALYWAVE_SMOKE=http://127.0.0.1:8420 xcrun simctl launch '$(IOS_SIM)' rest.daly.wave
	@echo "Журнал: $$(xcrun simctl get_app_container '$(IOS_SIM)' rest.daly.wave data)/Library/Application Support/rest.daly.wave/logs/DalyWave.log"

ios-test: ## симулятор: проверка окна входа, снимки — в design/shots/ios
	@bash apps/shell/scripts/xcodegen.sh
	set -o pipefail; xcodebuild -project $(IOS_PROJECT) -scheme dalywave-shell_iOS -configuration release \
	  -destination 'platform=iOS Simulator,name=$(IOS_SIM)' -derivedDataPath apps/shell/target/xcode test | tail -n 5

audio-test: ## аудиодвижок (crates/client): fmt, clippy, тесты в release
	cd crates/client && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --release

test-audio-server: ## тестовое аудио по HTTP с Range на :8420 (для ios-sim и стенда оболочки)
	@[ -f crates/client/audio/testdata/manifest.json ] || python3 crates/client/audio/tools/gen_audio.py
	python3 crates/client/audio/tools/serve.py --port 8420

design: ## статика макетов на :4400
	bun tools/serve.ts design 4400

api-types: ## TypeScript-типы интерфейса из OpenAPI сервера
	cd apps/ui && bun run api-types

sqlx-prepare: ## офлайн-данные запросов sqlx для сборки без базы
	cargo sqlx prepare --workspace -- --all-targets

test: ## все тесты (у сервера — свой Postgres этой копии: tools/test-pg.sh)
	tools/test-pg.sh cargo test --workspace
	@if [ -d apps/ui ]; then cd apps/ui && bun run typecheck; fi
	@if [ -d services/fetcher ]; then cd services/fetcher && uv run pytest -q; fi

lint: ## проверки стиля
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	@if [ -d apps/ui ]; then cd apps/ui && bun run lint; fi
	@if [ -d services/fetcher ]; then cd services/fetcher && uv run ruff check . && uv run ruff format --check .; fi

deploy: ## выложить текущий коммит на wave.daly.rest (сборка на сервере)
	bash infra/deploy.sh

fmt: ## отформатировать код
	cargo fmt --all
	@if [ -d apps/ui ]; then cd apps/ui && bun run lint --fix; fi
	@if [ -d services/fetcher ]; then cd services/fetcher && uv run ruff format .; fi

.PHONY: help up down db-reset api worker ml downloader ui sidecar app bump-version release-mac ios ios-build testflight testflight-tv ios-sim ios-test audio-test test-audio-server design api-types sqlx-prepare test lint deploy fmt
