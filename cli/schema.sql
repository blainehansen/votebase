-- this file is meant to be run by an admin role at polity creation time
-- this needs the ability to create roles to create new rulesets and their sub roles
-- and to create databases to do the migration checking
create role "votebase_server" with createrole createdb nosuperuser noinherit login password '{votebase_server_password}';

alter default privileges revoke all privileges on tables from PUBLIC;
alter default privileges revoke all privileges on sequences from PUBLIC;
alter default privileges revoke all privileges on functions from PUBLIC;
alter default privileges revoke all privileges on types from PUBLIC;
alter default privileges revoke all privileges on schemas from PUBLIC;
revoke all privileges on database "{db_database}" from PUBLIC;
grant connect on database "{db_database}" to PUBLIC;
revoke all privileges on parameter search_path from PUBLIC;

-- CREATE | CONNECT | TEMPORARY | TEMP
grant all privileges on database "{db_database}" to "votebase_server";
-- SET | ALTER SYSTEM
grant all privileges on parameter search_path to "votebase_server";
-- USAGE | CREATE
alter default privileges grant all privileges on schemas to "votebase_server";

drop schema public;
create schema votebase_catalog;

-- SELECT | INSERT | UPDATE | DELETE | TRUNCATE | REFERENCES | TRIGGER | MAINTAIN
alter default privileges in schema votebase_catalog grant all privileges on tables to "votebase_server";
-- USAGE | SELECT | UPDATE
alter default privileges in schema votebase_catalog grant all privileges on sequences to "votebase_server";
-- EXECUTE
alter default privileges in schema votebase_catalog grant all privileges on functions to "votebase_server";
-- USAGE
alter default privileges in schema votebase_catalog grant all privileges on types to "votebase_server";

create extension if not exists pgcrypto with schema votebase_catalog;

create table votebase_catalog.ruleset (
	full_path text primary key generated always as (case
		when parent_full_path is null then "name"
		else parent_full_path || '|' || "name"
	end) stored,
		-- constraint well_formed_path check (full_path = votebase_catalog.make_full_path(parent_full_path, "name"))
		-- default votebase_catalog.make_full_path(parent_full_path, "name"),
	parent_full_path text references votebase_catalog.ruleset(full_path) on delete cascade,
	"name" text not null constraint name_only_letters check ("name" similar to '[A-Za-z]+'),

	actions text[] not null,
	views text[] not null,
	constraint actions_views_different_names check (not (actions && views)),

	migrator_pass text not null default encode(votebase_catalog.gen_random_bytes(526), 'base64'),
	action_pass text not null default encode(votebase_catalog.gen_random_bytes(526), 'base64'),
	view_pass text not null default encode(votebase_catalog.gen_random_bytes(526), 'base64'),

	code text not null,
	db_schema text not null
);

-- create type fn_type as enum('Action', 'View');

-- create table votebase_catalog.ruleset_fn (
-- 	full_path text not null references votebase_catalog.ruleset(full_path) on delete cascade,
-- 	"name" text not null constraint name_only_letters check ("name" similar to '[A-Za-z]+'),
-- 	primary key (full_path, "name"),
-- 	"type" fn_type not null,
-- 	input_schema jsonb not null
-- 	-- trigger_on_slack bool not null default false,
-- 	-- trigger_on_github bool not null default false,
-- );

-- create table votebase_catalog.ruleset_replacements (
-- 	full_path text not null references votebase_catalog.ruleset(full_path) on delete cascade,

-- 	old_actions text[] not null,
-- 	old_views text[] not null,

-- 	old_db_schema text not null,
-- 	db_migration text not null,
-- 	old_code text not null
-- );

create unique index ruleset_single_null_parent
on votebase_catalog.ruleset((true))
where parent_full_path is null;

create table votebase_catalog.candidate_replacement_ruleset (
	id uuid primary key default gen_random_uuid(),
	candidate_for text not null references votebase_catalog.ruleset(full_path) on delete cascade,

	actions text[] not null,
	views text[] not null,
	constraint actions_views_different_names check (not (actions && views)),

	code text not null,
	db_schema text not null,
	db_migration text not null
);

-- create table votebase_catalog.candidate_child_ruleset (
-- 	id uuid primary key default gen_random_uuid(),
-- 	candidate_for text not null references votebase_catalog.ruleset(full_path) on delete cascade,

-- 	actions text[] not null,
-- 	views text[] not null,
-- 	constraint actions_views_different_names check (not (actions && views)),

-- 	code text not null,
-- 	db_schema text not null,
-- 	db_migration text not null
-- );


create or replace function votebase_catalog.insert_candidate_replacement(
	p_candidate_for text, p_actions text[], p_views text[],
	p_code text, p_db_schema text, p_db_migration text
) returns uuid as $$
declare
	candidate_id uuid;
begin
	if not exists (select 1 from votebase_catalog.ruleset where full_path = p_candidate_for) then
		raise exception 'ruleset % not found', p_candidate_for;
	end if;

	insert into votebase_catalog.candidate_replacement_ruleset (
		candidate_for,
		actions, views,
		code, db_schema, db_migration
	) values (
		p_candidate_for,
		p_actions, p_views,
		p_code, p_db_schema, p_db_migration
	)
	returning id into candidate_id;

	return candidate_id;
end;
$$ language plpgsql;


create or replace function votebase_catalog.apply_candidate(candidate_id uuid) returns text as $$
declare
	candidate votebase_catalog.candidate_replacement_ruleset;
begin
	select * into candidate
	from votebase_catalog.candidate_replacement_ruleset
	where id = candidate_id;

	if not found then
		raise exception 'candidate ruleset % not found', candidate_id;
	end if;

	update votebase_catalog.ruleset
	set
		actions = candidate.actions, views = candidate.views,
		code = candidate.code, db_schema = candidate.db_schema
	where full_path = candidate.candidate_for;

	delete from votebase_catalog.candidate_replacement_ruleset
	where id = candidate_id;

	-- TODO insert into votebase_catalog.ruleset_replacements

	-- TODO need to create child rulesets?

	return candidate.db_migration;
end;
$$ language plpgsql;


create table votebase_catalog.member (
	id uuid primary key default gen_random_uuid(),
	email text not null unique
);

-- create table votebase_catalog.member_to_ruleset (
-- 	member_id uuid not null references votebase_catalog.member(id) on delete cascade,
-- 	ruleset_full_path text not null references votebase_catalog.ruleset(full_path) on delete cascade,
-- 	primary key (member_id, ruleset_full_path)
-- );

create type votebase_catalog.granularity_enum as enum('Day', 'Week', 'Month', 'Year');

-- create table votebase_catalog.recurring_action (
-- 	full_path text not null references votebase_catalog.ruleset(full_path) on delete cascade,
-- 	"name" text not null constraint name_only_letters check ("name" similar to '[A-Za-z]+'),
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

create table votebase_catalog.detached_recurring_action (
	id uuid primary key default gen_random_uuid(),
	full_path text not null references votebase_catalog.ruleset(full_path) on delete cascade,
	description text not null,
	"start" timestamp not null,
	recurrence_granularity votebase_catalog.granularity_enum not null,
	recurrence_multiplier smallint not null check(recurrence_multiplier > 0),
	action_name text not null,
	action_arg json not null,
	executing bool not null default false,
	executed_count int not null default 0,
	next_scheduled_time timestamp not null generated always as ("start" + ((case recurrence_granularity
		when 'Day' then '1 day'::interval
		when 'Week' then '1 week'::interval
		when 'Month' then '1 month'::interval
		when 'Year' then '1 year'::interval
	end) * recurrence_multiplier * executed_count)) stored
);

create table votebase_catalog.detached_scheduled_action (
	id uuid primary key default gen_random_uuid(),
	full_path text not null references votebase_catalog.ruleset(full_path) on delete cascade,
	description text not null,
	scheduled_time timestamptz not null,
	action_name text not null,
	action_arg json not null,
	executing bool not null default false
);
