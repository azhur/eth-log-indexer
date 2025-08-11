.PHONY: init db-init db-reinit run run-release test lint

DATABASE_FILE_PATH = $(shell grep -m1 '^DATABASE_FILE_PATH=' .env 2>/dev/null | cut -d'=' -f2-)

init:
	cp .env.sample .env
	$(MAKE) db-init

db-init:
	@echo "Initializing SQLite database..."
	cargo install sqlx-cli --no-default-features --features sqlite
	mkdir -p "data"
	touch "$(DATABASE_FILE_PATH)"
	cargo sqlx migrate run

db-reinit:
	@echo "Deleting SQLite database..."
	rm -f "$(DATABASE_FILE_PATH)"
	$(MAKE) db-init

run:
	cargo run

run-release:
	cargo run --release

test:
	cargo test

lint:
	cargo clippy -- -D warnings

fmt-check:
	cargo fmt --all --manifest-path=./Cargo.toml -- --color=always --check 2>/dev/null
