# Wrappers so every app's commands run from the repository root.
#
# Cargo insists on being invoked inside its workspace, and `apps/api` is a
# Cargo workspace nested inside a JS one. Typing `cd apps/api && cargo …`
# forty times a day is the papercut this file removes. The `fe-` and `ds-`
# targets exist for the same reason on the JavaScript side.

API := apps/api
LANDING := @anakmobil/landing
# A scratch database for the sqlx cache. Empty by construction — see be-prepare.
PREPARE_URL := postgres://postgres:anakmobil@127.0.0.1:55432/anakmobil_prepare
TOKENS := @anakmobil/tokens
MOBILE := @anakmobil/mobile
MOBILE_DIR := apps/mobile

# Which platform the mb-run-* device builds target. `p=android` overrides.
p ?= ios

.DEFAULT_GOAL := help
.PHONY: help be-run be-web be-worker be-migrate be-fmt be-lint be-test be-cov be-audit be-boundary be-check \
        ds-build ds-check fe-dev fe-build fe-preview fe-check \
        mb-check mb-run-dev mb-run-preview mb-run-prod check

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

# Compile against the committed .sqlx cache, never the live database.
#
# Without this, `DATABASE_URL` being set makes the sqlx macros query the
# server and IGNORE the cache — so a stale cache passes here and fails on
# any machine without a database. It also makes the schema a build
# dependency, which is circular: `be-migrate` could not compile against an
# empty database, because the thing that creates the tables needs them to
# already exist.
#
# `?=` so `SQLX_OFFLINE=false make be-prepare` can regenerate the cache.
SQLX_OFFLINE ?= true

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

db-drop: ## Drop and recreate the database, then migrate — faster than db-reset
	@docker compose exec -T postgres psql -U postgres -q \
		-c 'DROP DATABASE IF EXISTS anakmobil WITH (FORCE)' -c 'CREATE DATABASE anakmobil'
	@$(MAKE) --no-print-directory be-migrate
	@echo 'empty. `make db-seed` adds the starter catalog.'

# Reference data only, and that distinction is the point.
#
# The repository rule is that nothing is seeded with fake data: no invented
# community counts, no fabricated activity, because the platform launches
# empty and the low-data state is designed as a primary experience rather
# than a fallback. A catalog of cars that genuinely exist is not that — it is
# reference data, and `POST /catalog/suggestions` already exists for the cars
# it is missing.
#
# So there is no `db-seed-users` and no fixture garage. Nothing here creates a
# person, a vehicle, a build, or a service record. Develop against the empty
# state, because that is what the first real user sees.
db-seed: ## Load the starter vehicle catalog — real cars only, no fabricated activity
	@test "$${APP_ENV:-development}" = development \
		|| { echo 'refusing: APP_ENV is $(APP_ENV), not development'; exit 1; }
	@docker compose exec -T postgres psql -U postgres -d anakmobil -q -v ON_ERROR_STOP=1 \
		< $(API)/seeds/catalog.sql

db-psql: ## Open a psql shell on the development database
	docker compose exec postgres psql -U postgres -d anakmobil

# `make dev` — every surface that exists, in one terminal.
#
# Stopping this cleanly took three attempts, and each failure is why a line
# below looks the way it does.
#
# `cargo run` execs the server as a CHILD, so killing cargo leaves
# `anakmobil` holding :8080. `set -m` puts each background job in its own
# process group and `kill -- -PID` then takes the whole subtree.
#
# That is still not enough for the landing server. Astro re-execs itself and
# the survivor ends up re-parented to launchd in a process group of its own
# — outside the group we just killed. Measured: PPID 1, PGID equal to its own
# pid. So the trap also sweeps the two ports this target is documented to
# own. Blunt, and precise enough: these are ports we started ourselves,
# seconds earlier.
#
# Without all of it, ctrl-c leaves a listener behind and the next `make dev`
# dies with "address already in use" — which would make this worse than
# opening two terminals by hand.
#
# awk rather than sed for the log prefix: BSD awk has fflush(), so lines
# appear as they happen instead of in 4KB bursts.
#
# Every comment here sits ABOVE the recipe on purpose. A `#` line inside a
# recipe is handed to the shell, and one ending in a backslash continues
# the comment onto the next line — silently swallowing the command that
# follows it. That is not hypothetical; it is how the first version of this
# target ran nothing at all while reporting success.
#
# When apps/mobile is scaffolded, add one more line to the group:
#   ( bun run --filter @anakmobil/mobile start 2>&1 | awk '{print "[mobile]  " $$0; fflush()}' ) &
dev: db-up ds-build ## Run every surface that exists — API, landing, and Metro
	@echo 'api      \033[36mhttp://localhost:8080\033[0m'
	@echo 'landing  \033[36mhttp://localhost:4321\033[0m'
	@echo 'metro    \033[36mhttp://localhost:8081\033[0m'
	@echo 'ctrl-c stops all'
	@echo
	@set -m; \
		trap 'kill -- -$$api -$$landing -$$mobile 2>/dev/null; for p in $$(lsof -ti tcp:8080 -ti tcp:4321 -ti tcp:8081 2>/dev/null); do kill $$p 2>/dev/null; done; wait 2>/dev/null' EXIT INT TERM; \
		( cd $(API) && cargo run --quiet --bin anakmobil -- web 2>&1 | awk '{print "[api]     " $$0; fflush()}' ) & api=$$!; \
		( bun run --filter $(LANDING) dev 2>&1 | awk '{print "[landing] " $$0; fflush()}' ) & landing=$$!; \
		( bun run --filter $(MOBILE) start 2>&1 | awk '{print "[mobile]  " $$0; fflush()}' ) & mobile=$$!; \
		wait

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

# Regenerate against a THROWAWAY EMPTY database, never the one you develop on.
#
# sqlx infers Postgres nullability by reading the query plan, and the plan
# changes with table statistics. Measured: with 546 vehicles and 1062 users in
# the table, `find_owned`'s four LEFT JOINs make the planner switch strategy
# and sqlx decides `v.id` is nullable — the macro then fails to compile against
# a schema that has not changed. The same check on an empty database passes.
#
# That alone would be an annoyance. What makes it dangerous is that
# `cargo sqlx prepare` CLEARS .sqlx before regenerating, so a failed run leaves
# an empty cache and breaks the offline build too — the one thing the cache
# exists to keep working.
#
# CI always has a fresh database, which is why this only ever bites locally.
# So: build one, use it, drop it.
be-prepare: ## Regenerate the committed .sqlx cache against a throwaway empty database
	@echo 'preparing against a scratch database, not $(shell echo $$DATABASE_URL | sed "s|.*/||")'
	@docker compose exec -T postgres psql -U postgres -q -c 'DROP DATABASE IF EXISTS anakmobil_prepare' -c 'CREATE DATABASE anakmobil_prepare'
	@cd $(API)/crates/runtime && DATABASE_URL=$(PREPARE_URL) sqlx migrate run >/dev/null
	cd $(API) && SQLX_OFFLINE=false DATABASE_URL=$(PREPARE_URL) cargo sqlx prepare --workspace -- --all-targets
	@docker compose exec -T postgres psql -U postgres -q -c 'DROP DATABASE anakmobil_prepare'

# Same reasoning as be-prepare: check against an empty database, because that
# is what CI has and what the cache was generated from.
be-sqlx-check: ## Fail if the committed .sqlx cache is stale (what CI runs)
	@docker compose exec -T postgres psql -U postgres -q -c 'DROP DATABASE IF EXISTS anakmobil_prepare' -c 'CREATE DATABASE anakmobil_prepare'
	@cd $(API)/crates/runtime && DATABASE_URL=$(PREPARE_URL) sqlx migrate run >/dev/null
	cd $(API) && SQLX_OFFLINE=false DATABASE_URL=$(PREPARE_URL) cargo sqlx prepare --workspace --check -- --all-targets
	@docker compose exec -T postgres psql -U postgres -q -c 'DROP DATABASE anakmobil_prepare'

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

# --- Mobile ------------------------------------------------------------------
#
# mb-check mirrors fe-check: it runs the workspace's own `check` script, which
# generates the Expo Router typed routes, then `tsc --noEmit`, then `expo lint`.
# The mb-run-* targets each pick a build profile — they set APP_VARIANT so
# app.config.ts resolves that variant's app id/name, and source the matching
# apps/mobile/.env.<variant> so EXPO_PUBLIC_API_URL is a real process-env var
# that babel inlines. `p=ios` (default) or `p=android` chooses the device.
#
# These recipes inherit the root .env like every recipe (`-include .env; export`
# above). They consume none of it, and only EXPO_PUBLIC_-prefixed vars ever reach
# the bundle — a backend secret has no such name — so nothing can leak here.
#
# mb-run-* build to a real device and need the owner's Xcode/Android Studio; they
# cannot run in CI, which only runs mb-check.

mb-check: ## Type-check and lint the mobile app (typed routes generated first)
	bun run --filter $(MOBILE) check
	@echo "mobile gate green"

mb-run-dev: ## Build+run the development variant on a device (p=ios|android)
	@set -a; [ -f $(MOBILE_DIR)/.env.development ] && . ./$(MOBILE_DIR)/.env.development; set +a; \
		cd $(MOBILE_DIR) && APP_VARIANT=development bunx expo run:$(p)

mb-run-preview: ## Build+run the preview variant on a device (p=ios|android)
	@set -a; [ -f $(MOBILE_DIR)/.env.preview ] && . ./$(MOBILE_DIR)/.env.preview; set +a; \
		cd $(MOBILE_DIR) && APP_VARIANT=preview bunx expo run:$(p)

mb-run-prod: ## Build+run the production variant on a device (p=ios|android)
	@set -a; [ -f $(MOBILE_DIR)/.env.production ] && . ./$(MOBILE_DIR)/.env.production; set +a; \
		cd $(MOBILE_DIR) && APP_VARIANT=production bunx expo run:$(p)

check: be-check fe-check mb-check ## Every gate in the repository
