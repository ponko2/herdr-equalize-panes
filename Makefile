SHELL := /bin/bash
.SHELLFLAGS := -euo pipefail -c
.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

.PHONY: all
all: install

.PHONY: build
build: ## Build the plugin binary.
	cargo build --release

.PHONY: test
test: ## Run the test suite.
	cargo test

.PHONY: install
install: build ## Build and register the plugin with Herdr.
	herdr plugin link $(CURDIR)

.PHONY: uninstall
uninstall: ## Unregister the plugin from Herdr.
	herdr plugin unlink ponko2.equalize-panes

.PHONY: clean
clean: ## Remove build artifacts.
	cargo clean
