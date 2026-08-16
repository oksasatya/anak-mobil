# Wrappers so every app's commands run from the repository root.
#
# Cargo insists on being invoked inside its workspace, and `apps/api` is a
# Cargo workspace nested inside a JS one. Typing `cd apps/api && cargo …`
# forty times a day is the papercut this file removes. The `fe-` and `ds-`
# targets exist for the same reason on the JavaScript side.

API := apps/api
LANDING := @anakmobil/landing
TOKENS := @anakmobil/tokens

.DEFAULT_GOAL := help
.PHONY: help be-run be-web be-worker be-migrate be-fmt be-lint be-test be-cov be-audit be-boundary be-check \
        ds-build ds-check fe-dev fe-build fe-preview fe-check check

# Load .env and hand every value to the recipes below.
#
# `-include` rather than `include`: a fresh clone has no .env yet, and the
# help target must still work. `export` then passes them to every recipe.
#
# This is not convenience. The integration tests return early and report
# PASSING when DATABASE_URL and REDIS_URL are absent, so a shell without them
# produces a full green board that executed nothing. Loading them here is what
# makes `make be-test` mean what it says.
-include .env
export

help: ## Show this help
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

db-up: ## Start Postgres (pgvector). Redis is assumed to be running locally
	docker compose up -d postgres
	@printf 'waiting for postgres'
	@until docker compose exec -T postgres pg_isready -U postgres -d anakmobil >/dev/null 2>&1; do printf '.'; sleep 1; done
	@echo ' ready on 127.0.0.1:55432'

db-up-all: ## Start Postgres and Redis, for a machine with no local Redis
	docker compose --profile redis up -d
	@printf 'waiting for postgres'
	@until docker compose exec -T postgres pg_isready -U postgres -d anakmobil >/dev/null 2>&1; do printf '.'; sleep 1; done
	@echo ' ready'

db-down: ## Stop the containers, keeping the data
	docker compose --profile redis down

db-reset: ## Stop and DELETE the database volume, then start clean
	docker compose --profile redis down -v
	$(MAKE) db-up

db-psql: ## Open a psql shell on the development database
	docker compose exec postgres psql -U postgres -d anakmobil

be-web: ## Run the API in its web role
	cd $(API) && cargo run --bin anakmobil -- web

be-worker: ## Run the API in its worker role
	cd $(API) && cargo run --bin anakmobil -- worker

be-migrate: ## Apply database migrations and exit
	cd $(API) && cargo run --bin anakmobil -- migrate

be-fmt: ## Format the backend
	cd $(API) && cargo fmt

be-lint: ## Lint the backend, warnings are errors
	cd $(API) && cargo clippy --all-targets --all-features -- -D warnings

be-test: ## Run backend tests
	cd $(API) && cargo test --workspace

be-cov: ## Backend coverage (requires cargo-llvm-cov)
	cd $(API) && cargo llvm-cov --workspace --summary-only

be-audit: ## Check dependencies for advisories (requires cargo-audit)
	cd $(API) && cargo audit

be-boundary: ## AM-350 AC2 — prove the domain crate has no framework dependency
	@cd $(API) && if cargo tree -p anakmobil-domain \
		| grep -qE '\b(axum|sqlx|reqwest|redis|tracing|tokio)\b'; then \
		echo "FAIL: a framework reached anakmobil-domain"; \
		cargo tree -p anakmobil-domain; \
		exit 1; \
	else \
		echo "OK: anakmobil-domain has no framework dependency"; \
	fi

be-check: be-fmt be-lint be-test be-boundary ## Everything the backend CI gate runs
	@echo "backend gate green"

# --- Design system -----------------------------------------------------------

ds-build: ## Regenerate the CSS artifacts from the token source
	bun run --filter $(TOKENS) build

ds-check: ## Regenerate the tokens and test them
	bun run --filter $(TOKENS) check

# --- Landing -----------------------------------------------------------------
#
# fe-build depends on ds-build: the landing imports the generated stylesheet,
# and a stale dist/ there means the page silently ships yesterday's palette.

fe-dev: ds-build ## Run the landing dev server
	bun run --filter $(LANDING) dev

fe-build: ds-build ## Build the landing site
	bun run --filter $(LANDING) build

fe-preview: ## Serve the built landing site (what Lighthouse must measure)
	bun run --filter $(LANDING) preview

fe-check: ds-check ## Type-check and build the landing site
	bun run --filter $(LANDING) gate
	@echo "landing gate green"

check: be-check fe-check ## Every gate in the repository
