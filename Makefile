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

ui: ## интерфейс на :3400 (проксирует /api на :8400)
	cd apps/ui && bun run dev

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

.PHONY: help up down db-reset api ui design api-types sqlx-prepare test lint deploy fmt
