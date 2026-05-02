create schema "{formatted_ruleset_schema}";
create extension if not exists pgcrypto with schema "{formatted_ruleset_schema}";

create role "{formatted_ruleset_role_migrator}" with nocreaterole nosuperuser nocreatedb noinherit nologin;
alter role "{formatted_ruleset_role_migrator}" set search_path to "{formatted_ruleset_schema}";
grant "{formatted_ruleset_role_migrator}" to "votebase_server_{db_name}";
create role "{formatted_ruleset_role_action}" with nocreaterole nosuperuser nocreatedb noinherit nologin;
alter role "{formatted_ruleset_role_action}" set search_path to "{formatted_ruleset_schema}";
grant "{formatted_ruleset_role_action}" to "votebase_server_{db_name}";
create role "{formatted_ruleset_role_view}" with nocreaterole nosuperuser nocreatedb noinherit nologin;
alter role "{formatted_ruleset_role_view}" set search_path to "{formatted_ruleset_schema}";
grant "{formatted_ruleset_role_view}" to "votebase_server_{db_name}";

-- https://www.postgresql.org/docs/current/ddl-priv.html

-- views
-- view role should not be able to modify data *at all*
-- use schema
grant usage on schema "{formatted_ruleset_schema}" to "{formatted_ruleset_role_view}";
-- leaving out: create

-- read tables
alter default privileges in schema "{formatted_ruleset_schema}"
	grant select on tables to "{formatted_ruleset_role_view}";
-- leaving out: insert, update, delete, truncate, references, trigger, maintain

-- read sequences
alter default privileges in schema "{formatted_ruleset_schema}"
	grant usage, select on sequences to "{formatted_ruleset_role_view}";
-- leaving out: update
-- leaving out functions! this ensures views can only read, and can't call functions that modify


-- actions
-- use schema
grant usage on schema "{formatted_ruleset_schema}" to "{formatted_ruleset_role_action}";
-- leaving out: create

-- write tables
alter default privileges in schema "{formatted_ruleset_schema}"
	grant select, insert, update, delete on tables to "{formatted_ruleset_role_action}";
-- leaving out: truncate, references, trigger, maintain

-- everything sequences
alter default privileges in schema "{formatted_ruleset_schema}"
	grant all privileges on sequences to "{formatted_ruleset_role_action}";
-- includes: usage, select, update

-- everything functions
alter default privileges in schema "{formatted_ruleset_schema}"
	grant all privileges on functions to "{formatted_ruleset_role_action}";
-- includes: execute


-- migrator
-- everything schema
grant all privileges on schema "{formatted_ruleset_schema}" to "{formatted_ruleset_role_migrator}";
-- includes: usage, create
-- note: the create privilege is for creating things *in* the schema, not to create schemas themselves

-- everything tables
alter default privileges in schema "{formatted_ruleset_schema}"
	grant all privileges on tables to "{formatted_ruleset_role_migrator}";
-- includes: select, insert, update, delete, truncate, references, trigger, maintain

-- everything sequences
alter default privileges in schema "{formatted_ruleset_schema}"
	grant all privileges on sequences to "{formatted_ruleset_role_migrator}";
-- includes: usage, select, update

-- everything functions
alter default privileges in schema "{formatted_ruleset_schema}"
	grant all privileges on functions to "{formatted_ruleset_role_migrator}";
-- includes: execute

-- everything types
alter default privileges in schema "{formatted_ruleset_schema}"
	grant all privileges on types to "{formatted_ruleset_role_migrator}";
-- includes: usage
