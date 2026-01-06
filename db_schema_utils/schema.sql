-- this file is meant to be run by an admin role at polity creation time
-- this needs the ability to create roles to create new rulesets and their sub roles
-- and to create databases to do the migration checking
create role "votebase_server_{db_name}" with createrole createdb nosuperuser noinherit login password '{votebase_server_password}';

alter default privileges revoke all privileges on tables from PUBLIC;
alter default privileges revoke all privileges on sequences from PUBLIC;
alter default privileges revoke all privileges on functions from PUBLIC;
alter default privileges revoke all privileges on types from PUBLIC;
alter default privileges revoke all privileges on schemas from PUBLIC;
revoke all privileges on database "{db_name}" from PUBLIC;
grant connect on database "{db_name}" to PUBLIC;
revoke all privileges on parameter search_path from PUBLIC;

-- CREATE | CONNECT | TEMPORARY | TEMP
grant all privileges on database "{db_name}" to "votebase_server_{db_name}";
-- SET | ALTER SYSTEM
grant all privileges on parameter search_path to "votebase_server_{db_name}";
-- USAGE | CREATE
alter default privileges grant all privileges on schemas to "votebase_server_{db_name}";

drop schema public;
create schema votebase_catalog;

-- SELECT | INSERT | UPDATE | DELETE | TRUNCATE | REFERENCES | TRIGGER | MAINTAIN
alter default privileges in schema votebase_catalog grant all privileges on tables to "votebase_server_{db_name}";
-- USAGE | SELECT | UPDATE
alter default privileges in schema votebase_catalog grant all privileges on sequences to "votebase_server_{db_name}";
-- EXECUTE
alter default privileges in schema votebase_catalog grant all privileges on functions to "votebase_server_{db_name}";
-- USAGE
alter default privileges in schema votebase_catalog grant all privileges on types to "votebase_server_{db_name}";

create extension if not exists pgcrypto with schema votebase_catalog;


create function votebase_catalog.valid_name(name text) returns boolean as $$
	begin
		return name similar to '[A-Za-z0-9]+';
	end;
$$ language plpgsql immutable;


create type votebase_catalog.db_uses_function_struct as (
	ruleset_path text, object_name text,
	is_action boolean, return_type text, param_types text[]
);

create type votebase_catalog.column_use_kind as enum('Query', 'Reference', 'Both');
create type votebase_catalog.db_uses_column_struct as (name text, typ text, can_null boolean, use_kind votebase_catalog.column_use_kind);
create type votebase_catalog.db_uses_table_struct as (
	ruleset_path text, object_name text,
	columns votebase_catalog.db_uses_column_struct[]
);

create type votebase_catalog.fn_type as enum('Action', 'View');
create type votebase_catalog.ruleset_fn_raw as (
	"name" text, fn_type votebase_catalog.fn_type
	-- input_schema jsonb, output_schema jsonb
);
create domain votebase_catalog.ruleset_fn as votebase_catalog.ruleset_fn_raw
	not null
	check ((VALUE)."name" is not null)
	check (votebase_catalog.valid_name((VALUE)."name"))
	check ((VALUE).fn_type is not null)
	-- check ((VALUE).input_schema is not null)
	-- check ((VALUE).output_schema is not null)
;
create function votebase_catalog.check_fns_different_names(fns votebase_catalog.ruleset_fn[]) returns boolean as $$
	begin
		return not exists (
			select 1
			from unnest(fns) as fns
			group by (fns)."name"
			having count(*) > 1
		);
	end;
$$ language plpgsql immutable;

create function votebase_catalog.has_fn(fns votebase_catalog.ruleset_fn[], name text, type votebase_catalog.fn_type) returns boolean as $$
	begin
		return exists (
			select 1
			from unnest(fns) as fns
			where (fns)."name" = name and (fns).fn_type = type
		);
	end;
$$ language plpgsql immutable;

create function votebase_catalog.filter_fns(fns votebase_catalog.ruleset_fn[], type votebase_catalog.fn_type) returns text[] as $$
	begin
		return array (
			select (fns)."name"
			from unnest(fns) as fns
			where (fns).fn_type = type
		);
	end;
$$ language plpgsql immutable;


create table votebase_catalog.ruleset (
	full_path text primary key generated always as (case
		when parent_full_path is null then "name"
		else parent_full_path || '|' || "name"
	end) stored,
	parent_full_path text references votebase_catalog.ruleset(full_path) on delete cascade,
	"name" text not null constraint name_only_letters check (votebase_catalog.valid_name("name")),

	ts_code text not null,
	db_schema text not null,

	fns votebase_catalog.ruleset_fn[] not null
		constraint fns_different_names check (votebase_catalog.check_fns_different_names(fns)),
	db_uses_functions votebase_catalog.db_uses_function_struct[] not null,
	db_uses_tables votebase_catalog.db_uses_table_struct[] not null,

	migrator_pass text not null default encode(votebase_catalog.gen_random_bytes(526), 'base64'),
	action_pass text not null default encode(votebase_catalog.gen_random_bytes(526), 'base64'),
	view_pass text not null default encode(votebase_catalog.gen_random_bytes(526), 'base64')
);

create function votebase_catalog.drop_ruleset_schema_on_delete() returns trigger as $$
begin
	execute 'drop schema "ruleset:' || OLD.full_path || '" cascade';
	execute 'drop role "role:' || OLD.full_path || '|migrator"';
	execute 'drop role "role:' || OLD.full_path || '|action"';
	execute 'drop role "role:' || OLD.full_path || '|view"';
	return OLD;
end;
$$ language plpgsql;
create trigger votebase_catalog_trigger_drop_ruleset_schema_on_delete
after delete on votebase_catalog.ruleset for each row
execute function votebase_catalog.drop_ruleset_schema_on_delete();

create unique index votebase_catalog_ruleset_single_null_parent
on votebase_catalog.ruleset((true))
where parent_full_path is null;


create table votebase_catalog.candidate_replacement_ruleset (
	id uuid primary key default gen_random_uuid(),
	candidate_for text not null references votebase_catalog.ruleset(full_path) on delete cascade,

	bundled_ruleset jsonb not null

	-- ts_code text not null,
	-- db_schema text not null,
	-- db_migration text not null,

	-- fns votebase_catalog.ruleset_fn[] not null
	-- 	constraint fns_different_names check (votebase_catalog.check_fns_different_names(fns)),
	-- db_uses_functions votebase_catalog.db_uses_function_struct[] not null,
	-- db_uses_tables votebase_catalog.db_uses_table_struct[] not null
);


create table votebase_catalog.member (
	id uuid primary key default gen_random_uuid(),
	email text not null unique
);

-- create table votebase_catalog.member_to_ruleset (
-- 	member_id uuid not null references votebase_catalog.member(id) on delete cascade,
-- 	ruleset_full_path text not null references votebase_catalog.ruleset(full_path) on delete cascade,
-- 	primary key (member_id, ruleset_full_path)
-- );

-- create type votebase_catalog.granularity_enum as enum('Day', 'Week', 'Month', 'Year');

-- create table votebase_catalog.static_recurring_action (
-- 	full_path text not null references votebase_catalog.ruleset(full_path) on delete cascade,
-- 	"name" text not null constraint name_only_letters check (votebase_catalog.valid_name("name")),
-- 	primary key (full_path, "name"),
-- 	description text not null,
-- 	"start" timestamp not null,
-- 	recurrence_granularity votebase_catalog.granularity_enum not null,
-- 	recurrence_multiplier smallint not null check(recurrence_multiplier > 0),
-- 	action_name text not null,
-- 	action_arg json not null,
-- 	executing bool not null default false,
-- 	executed_count int not null default 0,
-- 	next_scheduled_time timestamp not null generated always as ("start" + ((case recurrence_granularity
-- 		when 'Day' then '1 day'::interval
-- 		when 'Week' then '1 week'::interval
-- 		when 'Month' then '1 month'::interval
-- 		when 'Year' then '1 year'::interval
-- 	end) * recurrence_multiplier * executed_count)) stored
-- );

-- create table votebase_catalog.detached_recurring_action (
-- 	id uuid primary key default gen_random_uuid(),
-- 	full_path text not null references votebase_catalog.ruleset(full_path) on delete cascade,
-- 	description text not null,
-- 	"start" timestamp not null,
-- 	recurrence_granularity votebase_catalog.granularity_enum not null,
-- 	recurrence_multiplier smallint not null check(recurrence_multiplier > 0),
-- 	action_name text not null,
-- 	action_arg json not null,
-- 	executing bool not null default false,
-- 	executed_count int not null default 0,
-- 	next_scheduled_time timestamp not null generated always as ("start" + ((case recurrence_granularity
-- 		when 'Day' then '1 day'::interval
-- 		when 'Week' then '1 week'::interval
-- 		when 'Month' then '1 month'::interval
-- 		when 'Year' then '1 year'::interval
-- 	end) * recurrence_multiplier * executed_count)) stored
-- );

-- create table votebase_catalog.detached_scheduled_action (
-- 	id uuid primary key default gen_random_uuid(),
-- 	full_path text not null references votebase_catalog.ruleset(full_path) on delete cascade,
-- 	description text not null,
-- 	scheduled_time timestamptz not null,
-- 	action_name text not null,
-- 	action_arg json not null,
-- 	executing bool not null default false
-- );


create type votebase_catalog.rough_column_struct as (name text, typ text, not_null boolean);

create function votebase_catalog.format_fn_args(proargtypes oidvector) returns text[] as $$
	begin
		return array (
			select pg_catalog.format_type(p.arg_type_oid, null)
			from unnest(proargtypes::oid[])
				with ordinality as p(arg_type_oid, ordinality)
		);
	end;
$$ language plpgsql immutable;

create function votebase_catalog.unformat_schema_name(schema_name text) returns text as $$
	begin
		return case
			when starts_with('ruleset:', schema_name) then substring(schema_name from 9)
			else schema_name
		end;
	end;
$$ language plpgsql immutable;
