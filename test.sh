PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'drop database dev_db' | cat
PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'create database dev_db' | cat
cargo run -p votebase_cli -- "postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db"
cargo test
