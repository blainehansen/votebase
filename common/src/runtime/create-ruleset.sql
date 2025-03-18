create schema "ruleset:{full_path}";

create role "role:{full_path}|migrator" with nosuperuser nocreatedb nocreaterole noinherit login password '{migrator_pass}';
alter role "role:{full_path}|migrator" set search_path to "ruleset:{full_path}";
create role "role:{full_path}|action" with nosuperuser nocreatedb nocreaterole noinherit login password '{action_pass}';
alter role "role:{full_path}|action" set search_path to "ruleset:{full_path}";
create role "role:{full_path}|view" with nosuperuser nocreatedb nocreaterole noinherit login password '{view_pass}';
alter role "role:{full_path}|view" set search_path to "ruleset:{full_path}";

-- https://www.postgresql.org/docs/current/ddl-priv.html

-- views
-- view role should not be able to modify data *at all*
-- use schema
grant usage on schema "ruleset:{full_path}" to "role:{full_path}|view";
-- leaving out: create
-- read tables
alter default privileges in schema "ruleset:{full_path}"
grant select on tables to "role:{full_path}|view";
-- leaving out: insert, update, delete, truncate, references, trigger, maintain
-- read sequences
alter default privileges in schema "ruleset:{full_path}"
grant usage, select on sequences to "role:{full_path}|view";
-- leaving out: update
-- leaving out functions! this ensures views can only read, and can't call functions that modify


-- actions
-- use schema
grant usage on schema "ruleset:{full_path}" to "role:{full_path}|action";
-- leaving out: create
-- write tables
alter default privileges in schema "ruleset:{full_path}"
grant select, insert, update, delete on tables to "role:{full_path}|action";
-- leaving out: truncate, references, trigger, maintain
-- everything sequences
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on sequences to "role:{full_path}|action";
-- includes: usage, select, update
-- everything functions
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on functions to "role:{full_path}|action";
-- includes: execute


-- migrator
-- everything schema
grant all privileges on schema "ruleset:{full_path}" to "role:{full_path}|migrator";
-- includes: usage, create
-- this is the key
-- everything tables
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on tables to "role:{full_path}|migrator";
-- includes: select, insert, update, delete, truncate, references, trigger, maintain
-- everything sequences
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on sequences to "role:{full_path}|migrator";
-- includes: usage, select, update
-- everything functions
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on functions to "role:{full_path}|migrator";
-- includes: execute
-- everything types
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on types to "role:{full_path}|migrator";
-- includes: usage
