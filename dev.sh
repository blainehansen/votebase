set -euo pipefail

# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db -f dev.sql

# we need two schemas, one of which we just create
# the other which we create a role to create, so this role needs to have all its permissions stripped
# then we selectively grant the abilities for the second to build the second schema, and reference the first
# then we muck around with things in the first to see if we're rejected, or if foreign keys still exist
# but most importantly what state the permissions are in, whether the grant we gave to the second schema role has been destroyed or changed or what

# first create two schemas, and roles for them that have no default permissions
# then grant one of them the ability to reference some interesting columns
# then use those two roles to create

PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db << EOF
	drop schema if exists used cascade;
	drop schema if exists b cascade;
	drop role if exists "b|role";

	create schema used
		create table t(id integer primary key, s bool not null);

	create schema b;
	create role "b|role" with nocreaterole nosuperuser nocreatedb noinherit login password 'b';
	alter role "b|role" set search_path to b;
	alter default privileges revoke all privileges on tables from "b|role";
	alter default privileges revoke all privileges on sequences from "b|role";
	alter default privileges revoke all privileges on functions from "b|role";
	alter default privileges revoke all privileges on types from "b|role";
	alter default privileges revoke all privileges on schemas from "b|role";

	grant all privileges on schema b to "b|role";
	alter default privileges in schema b
		grant all privileges on tables to "b|role";
	alter default privileges in schema b
		grant all privileges on sequences to "b|role";
	alter default privileges in schema b
		grant all privileges on functions to "b|role";
	alter default privileges in schema b
		grant all privileges on types to "b|role";

	grant usage on schema used to "b|role";
	grant references (id) on table t in schema used to "b|role";
EOF

PGPASSWORD='b' psql -U 'b|role' -h localhost dev_db << EOF
	create table b(i integer references used.t(id));
EOF



# cargo test -p votebase_common -- --nocapture
# cargo test -p votebase_cli -- --nocapture
# cargo run -p votebase_cli -- --ruleset-dir test-rulesets/yes-or-no/ dev

# cargo run -p votebase_cli -- \
# 	--db-url "postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db" \
# 	--ruleset-dir rulesets/three-yes-proposals \
# 	dev

# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db -c 'create table yo(id uuid primary key)' | cat
# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db -c '\dt' | cat

# PGPASSWORD='dev_admin_password' pg_dump dev_db -h localhost -U dev_admin_user \
# 	--schema-only \
# 	--format=custom --compress=none \
# 	--file=db_archive.local -v







# podman run --name test-postgres \
# 	--env POSTGRES_PASSWORD=dev_admin_password \
# 	--env POSTGRES_USER=dev_admin_user \
# 	--env POSTGRES_DB=dev_db \
# 	-p 5432:5432 \
# 	-it --rm \
# 	-v "$(pwd):/my_archive_data" \
# 	docker.io/library/postgres:latest
	# --detach --rm \

# podman exec -it test-postgres pg_dump -d dev_db -U dev_admin_user -f /my_archive_data/db_archive.test.local


# podman exec -it test-postgres pg_restore --help

# podman stop test-postgres

# # --network container:test-postgres
# podman run --rm votebase-dbdiff \
# 	--with-privileges \
# 	'postgresql://dev_admin_user:dev_admin_password@localhost:5432/dev_db' \
# 	'postgresql://dev_admin_user:dev_admin_password@localhost:5432/dev_db'





# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'create database db_from' | cat
# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'create database db_to' | cat


# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost db_from <<EOF
# create schema secret_stuff
# 	create table secret_table (secret_column text);

# create schema actual_thing
# 	create table if not exists users (
# 		id serial primary key
# 	);
# EOF

# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost db_to <<EOF
# create schema actual_thing
# 	create table if not exists users (
# 		id serial primary key,
# 		name text not null,
# 		email text unique
# 	);
# EOF


# podman build -t votebase-dbdiff -f Containerfile.votebase-dbdiff .
# podman run --rm --network host votebase-dbdiff \
# 	--with-privileges \
# 	--schema actual_thing \
# 	'postgresql://dev_admin_user:dev_admin_password@localhost:5432/db_from' \
# 	'postgresql://dev_admin_user:dev_admin_password@localhost:5432/db_to'







# podman run -it --rm -v "$(pwd)/rulesets/accept-any:/workspace/ruleset" --entrypoint="/bin/bash" votebase-tsc

# podman build -t votebase-tsc -f Containerfile.votebase-tsc .

# podman run --rm -v $(pwd):/workspace votebase-tsc --noEmit --project tsconfig.dev.json

# podman run --name clorinde_postgres -p 5432:5432 \
# 	-e POSTGRES_DB=dev_db -e POSTGRES_USER=dev_admin_user -e POSTGRES_PASSWORD=dev_admin_password \
# 	docker.io/library/postgres:latest



# podman run --name clorinde_postgres -p 5432:5432 \
# 	-e POSTGRES_DB=dev_db -e POSTGRES_USER=dev_admin_user -e POSTGRES_PASSWORD=dev_admin_password \
# 	docker.io/library/postgres:latest

# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c 'select 1' | cat

# podman exec clorinde_postgres pg_isready

# podman stop clorinde_postgres; podman rm -v clorinde_postgres


# # podman build -t uv-with-python .
# podman run --rm -it \
# 	-v uv-cache:/root/.cache/uv \
# 	-v $(pwd):/workspace \
# 	uv-with-python \
# 	tool run -p 3.11 --with psycopg2-binary --with setuptools migra --help


# podman run --rm -it \
#   -v uv-cache:/home/blaine/.cache/uv \
#   python:3.11-slim \
#   sh -c "pip install uv && uv tool run --with psycopg2-binary --with setuptools migra --help"


# podman run --rm -it \
# 	-v uv-cache:/home/blaine/.cache/uv \
# 	-e UV_NO_MANAGED_PYTHON=true \
# 	-e UV_SYSTEM_PYTHON=true \
# 	ghcr.io/astral-sh/uv:latest \
# 	tool run -p 3.11 --with psycopg2-binary --with setuptools "migra --help"

	# sh -c "uv tool run "


# https://hub.docker.com/r/mgoltzsche/podman
# docker run --privileged mgoltzsche/podman:minimal docker run alpine:latest echo hello from nested container


# podman run --privileged quay.io/podman/stable podman run ubi8 echo hello


# podman run --privileged -v ./mycontainers:/var/lib/containers quay.io/podman/stable podman run ubi8 echo hello
