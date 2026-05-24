set -euo pipefail

PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db << EOF

create role a;
select 1 from pg_roles where rolname = 'a';

begin;

create role b;
select 1 from pg_roles where rolname = 'b';

rollback;

select 1 from pg_roles where rolname = 'b';

EOF

# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost '{db_name}' -f db_schema_utils/schema.sql
# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost '{db_name}' -f dev.sql | cat
