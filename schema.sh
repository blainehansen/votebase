PGPASSWORD='dev_password' psql -U dev_user -h localhost postgres -c 'drop database dev_db' | cat
PGPASSWORD='dev_password' psql -U dev_user -h localhost postgres -c 'create database dev_db' | cat
PGPASSWORD='dev_password' psql -U dev_user -h localhost dev_db -f ./schema.sql | cat
DATABASE_URL="postgres://dev_user:dev_password@localhost:5432/dev_db" cargo sqlx prepare -- --all-targets --all-features
