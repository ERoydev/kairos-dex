# Running Migrator CLI

- Generate a new migration file
    ```sh
    cargo run -- generate MIGRATION_NAME
    ```
- Apply all pending migrations
    ```sh
    cargo run
    ```
    ```sh
    cargo run -- up
    ```
- Apply first 10 pending migrations
    ```sh
    cargo run -- up -n 10
    ```
- Rollback last applied migrations
    ```sh
    cargo run -- down
    ```
- Rollback last 10 applied migrations
    ```sh
    cargo run -- down -n 10
    ```
- Drop all tables from the database, then reapply all migrations
    ```sh
    cargo run -- fresh
    ```
- Rollback all applied migrations, then reapply all migrations
    ```sh
    cargo run -- refresh
    ```
- Rollback all applied migrations
    ```sh
    cargo run -- reset
    ```
- Check the status of all migrations
    ```sh
    cargo run -- status
    ```


# Workflow

## Reset on clean

To reset the local dev database to a clean state (drop all tables, reapply all migrations), run from the repo root:
```bash
cargo db-fresh
```
This is a cargo alias defined in `.cargo/config.toml` for `cargo run --manifest-path migration/Cargo.toml -- fresh`. It picks up `DATABASE_URL` from the repo-root `.env` automatically.

## Migration 

1. Define migration in Rust (schemas)
2. Register it in `migration/src/lib.rs`:
3. Run the migration:
```bash
sea-orm-cli migrate up -u postgres://user:pass@localhost:5432/kairos_indexer
```
4. Generate the entity struct (model):
```bash
sea-orm-cli generate entity \
  -u postgres://user:pass@localhost:5432/kairos_indexer \
  -o src/db/entities \
  --with-serde both
```
5. I can use that entity model in my rust indexer code