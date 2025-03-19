set -euxo pipefail

# start over
PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'drop database dev_db' | cat
PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'create database dev_db' | cat

# bootstrap the database with the schema and root ruleset
cargo run -p votebase_cli -- "postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db"

# prepare queries
DATABASE_URL="postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db" cargo sqlx prepare --workspace -- --all-targets --all-features

# run the server
cargo run -p votebase_server
