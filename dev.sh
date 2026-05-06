set -euo pipefail

PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db << EOF
	create database "{db_name}";
EOF

PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost '{db_name}' -f db_schema_utils/schema.sql
PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost '{db_name}' -f dev.sql | cat
