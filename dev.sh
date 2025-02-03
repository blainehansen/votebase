# curl -v "http://localhost:8080/view/some?hmmm=y"

PGPASSWORD='dev_password' psql -U dev_user -h localhost postgres -c 'drop database dev_db' | cat
PGPASSWORD='dev_password' psql -U dev_user -h localhost postgres -c 'create database dev_db' | cat
# PGPASSWORD='dev_password' psql -U dev_user -h localhost dev_db -c 'drop schema if exists public cascade' | cat
PGPASSWORD='dev_password' psql -U dev_user -h localhost dev_db -f ./schema.sql | cat


# PGPASSWORD='dev_password' psql -U dev_user -h localhost dev_db -f ./ruleset-examples/three-yes/schema.sql | cat
