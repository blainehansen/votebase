PGPASSWORD='dev_password' psql -U dev_user -h localhost postgres -c 'drop database dev_db' | cat
PGPASSWORD='dev_password' psql -U dev_user -h localhost postgres -c 'create database dev_db' | cat
PGPASSWORD='dev_password' psql -U dev_user -h localhost dev_db -f ./schema.sql | cat
PGPASSWORD='dev_password' psql -U dev_user -h localhost dev_db -f ./schema.dev.sql | cat
