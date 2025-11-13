set -euxo pipefail

# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'drop database dev_db' | cat
# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'create database dev_db' | cat
# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'drop role if exists "votebase_server"' | cat
# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db -f ./bootstrap_cli/schema.sql | cat

# # boo can't do this because the schema has params in it
# clorinde schema common/schema.sql \
# 	--queries-path common/queries \
# 	--destination queries \
# 	--async true \
# 	--podman true

# clorinde live \
# 	--async true \
# 	--queries-path ./queries-sql \
# 	--destination ./queries \
# 	"postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db"
