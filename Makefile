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

help: ## Show this help
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

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
	npm run build --workspace $(TOKENS)

ds-check: ## Regenerate the tokens and test them
	npm run check --workspace $(TOKENS)

# --- Landing -----------------------------------------------------------------
#
# fe-build depends on ds-build: the landing imports the generated stylesheet,
# and a stale dist/ there means the page silently ships yesterday's palette.

fe-dev: ds-build ## Run the landing dev server
	npm run dev --workspace $(LANDING)

fe-build: ds-build ## Build the landing site
	npm run build --workspace $(LANDING)

fe-preview: ## Serve the built landing site (what Lighthouse must measure)
	npm run preview --workspace $(LANDING)

fe-check: ds-check ## Type-check and build the landing site
	npm run gate --workspace $(LANDING)
	@echo "landing gate green"

check: be-check fe-check ## Every gate in the repository
