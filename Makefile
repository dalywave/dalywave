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

worker: ## исполнитель очереди задач (скачивание, обложки, подписки)
	cargo run -p dalywave-server --bin dalywave-worker

# Аудио качает этот Mac: YouTube не отдаёт его серверу. Токен dwk_… — из админки («Источники»):
# TOKEN=… или файл ~/.config/dalywave/downloader-token. В автозапуск: services/fetcher/launchd/install.sh
SERVER ?= https://wave.daly.rest
downloader: ## загрузчик на этом Mac: make downloader [SERVER=http://localhost:8400] [TOKEN=dwk_…]
	@FETCHER_WORKER_TOKEN="$(TOKEN)" bash services/fetcher/launchd/run.sh "$(SERVER)"

ui: ## интерфейс на :3400 (проксирует /api на :8400)
	cd apps/ui && bun run dev

# Подпись: Developer ID из связки ключей (иначе ad-hoc), ключ обновлений — вне репозитория,
# пароль к нему — в связке ключей (apps/shell/README.md). CI=true: DMG без AppleScript к Finder.
# Свои ключи сборки: make app TAURI_ARGS='--debug --bundles app'
UPDATER_KEY ?= $(HOME)/.tauri/dalywave-updater.key
app: ## приложение macOS: DalyWave.app и .dmg в apps/shell/target/release/bundle
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

test: ## все тесты
	cargo test --workspace
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

.PHONY: help up down db-reset api worker downloader ui app ios ios-build ios-sim ios-test audio-test test-audio-server design api-types sqlx-prepare test lint deploy fmt
