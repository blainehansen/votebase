# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'drop database dev_db' | cat
# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'create database dev_db' | cat
# cargo run -p votebase_cli -- "postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db"

# clorinde live \
# 	--async true \
# 	--queries-path ./queries-sql \
# 	--destination ./queries \
# 	"postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db"

# cargo test -p votebase_common run_function_basics -- --nocapture
# cargo test -p votebase_common -- --nocapture

cargo run -p votebase_cli "postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db" 'q.local'
