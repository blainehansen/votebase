set -euxo pipefail

# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'drop database dev_db' | cat
# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'create database dev_db' | cat
# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'drop role if exists "votebase_server"' | cat
# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db -f ./cli/schema.sql | cat

clorinde live \
	--async true \
	--queries-path ./queries-sql \
	--destination ./queries \
	"postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db"
