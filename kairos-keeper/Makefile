DEV_DB_URL := sqlite://./dev.db?mode=rwc

.PHONY: run dev build build-dev check reset-db db-shell seed

## Prod-style run: default features, postgres from .env
run:
	cargo run

## Dev run: sqlite driver + auto-migrate, dev.db file, no postgres needed
dev:
	RUST_LOG=info DATABASE_URL="$(DEV_DB_URL)" cargo run --features dev-db

build:
	cargo build

build-dev:
	cargo build --features dev-db

check:
	cargo check --features dev-db

## Wipe local dev db (re-created automatically on next `make dev`)
reset-db:
	rm -f dev.db dev.db-shm dev.db-wal

## Open the dev db in the sqlite3 CLI
db-shell:
	sqlite3 dev.db

## Drop mock rows (market, position, funding update, lp pool/event) into dev.db
seed:
	DATABASE_URL="$(DEV_DB_URL)" cargo run --features dev-db --bin seed
