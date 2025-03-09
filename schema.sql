drop schema public;

create schema votebase_catalog;
create extension if not exists pgcrypto with schema votebase_catalog;

create table votebase_catalog.ruleset (
	full_path text primary key constraint well_formed_path check (case
		when parent_full_path is null then full_path = "name"
		else full_path = parent_full_path || '|' || "name"
	end),
	"name" text not null constraint name_only_letters check ("name" similar to '[A-Za-z]+'),
	parent_full_path text references votebase_catalog.ruleset(full_path),

	actions text[] not null,
	views text[] not null,
	constraint actions_views_different_names check (not (actions && views)),
	action_pass text not null default encode(votebase_catalog.gen_random_bytes(526), 'base64'),
	view_pass text not null default encode(votebase_catalog.gen_random_bytes(526), 'base64'),

	code text not null,
	db_schema text not null
);

-- create table votebase_catalog.ruleset_replacements (
-- 	full_path text not null references votebase_catalog.ruleset(full_path),

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
	candidate_for text not null references votebase_catalog.ruleset(full_path),

	actions text[] not null,
	views text[] not null,
	constraint actions_views_different_names check (not (actions && views)),

	code text not null,
	db_schema text not null,
	db_migration text not null
);

-- create table votebase_catalog.candidate_child_ruleset (
-- 	id uuid primary key default gen_random_uuid(),
-- 	candidate_for text not null references votebase_catalog.ruleset(full_path),

-- 	actions text[] not null,
-- 	views text[] not null,
-- 	constraint actions_views_different_names check (not (actions && views)),

-- 	code text not null,
-- 	db_schema text not null,
-- 	db_migration text not null
-- );


-- create table votebase_catalog.member (
-- 	id uuid primary key,
-- 	email text not null,
-- 	display_name text not null
-- );

-- create table votebase_catalog.member_to_ruleset (
-- 	member_id uuid not null references votebase_catalog.member(id),
-- 	ruleset_full_path text not null references votebase_catalog.ruleset(full_path),
-- 	primary key (member_id, ruleset_full_path)
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
	where full_path = v_candidate.candidate_for;

	delete from votebase_catalog.candidate_replacement_ruleset
	where case
		when candidate.parent_full_path is null then parent_full_path is null
		else candidate.parent_full_path = parent_full_path
	end;

	-- TODO insert into votebase_catalog.ruleset_replacements

	-- TODO need to create child rulesets?

	return candidate.db_migration;
end;
$$ language plpgsql;



-- alter default privileges in schema votebase_catalog revoke all privileges on tables;
-- alter default privileges in schema votebase_catalog revoke all privileges on sequences;
-- alter default privileges in schema votebase_catalog revoke all privileges on functions;
-- alter default privileges in schema votebase_catalog revoke all privileges on types;
-- alter default privileges in schema votebase_catalog revoke all privileges on schemas;
-- alter default privileges revoke all privileges on database dev_db;
