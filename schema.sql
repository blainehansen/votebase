create schema votebase_catalog;

create table votebase_catalog.ruleset (
	full_path text primary key constraint well_formed_path check (case
		when parent_full_path is null then full_path = "name"
		else full_path = parent_full_path || '|' || "name"
	end),
	"name" text not null,
	parent_full_path text references votebase_catalog.ruleset(full_path),

	actions text[] not null,
	views text[] not null,
	constraint actions_views_different_names check (not (actions && views)),
	action_pass text not null,
	view_pass text not null,

	code text not null,
	db_schema text not null,
	db_migration text not null
);

create unique index ruleset_single_null_parent
on votebase_catalog.ruleset((true))
where parent_full_path is null;

create table votebase_catalog.candidate_ruleset (
	id uuid primary key default gen_random_uuid(),
	candidate_for text not null references votebase_catalog.ruleset(full_path),

	actions text[] not null,
	views text[] not null,
	constraint actions_views_different_names check (not (actions && views)),

	code text not null,
	db_schema text not null,
	db_migration text not null
);


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

create or replace procedure votebase_catalog.apply_candidate(candidate_id uuid) as $$
declare
	candidate votebase_catalog.candidate_ruleset;
begin
	select * into candidate
	from votebase_catalog.candidate_ruleset
	where id = candidate_id;

	if not found then
		raise exception 'candidate ruleset % not found', candidate_id;
	end if;

	update votebase_catalog.ruleset
	set
		actions = candidate.actions, views = candidate.views,
		code = candidate.code, db_schema = candidate.db_schema, db_migration = candidate.db_migration
	where full_path = v_candidate.candidate_for;

	delete from votebase_catalog.candidate_ruleset
	where case
		when candidate.parent_full_path is null then parent_full_path is null
		else candidate.parent_full_path = parent_full_path
	end;

	-- TODO need to create child rulesets?
end;
$$ language plpgsql;


create or replace procedure votebase_catalog.insert_ruleset(
	p_parent_full_path text, p_name text,
	p_action_pass text, p_actions text[], p_view_pass text, p_views text[],
	p_code text, p_db_schema text, p_db_migration text
) as $$
begin
	insert into votebase_catalog.ruleset (
		full_path,
		parent_full_path, "name",
		action_pass, actions, view_pass, views,
		code, db_schema, db_migration
	) values (
		case
			when p_parent_full_path is null then p_name
			else p_parent_full_path || '|' || p_name
		end,
		p_parent_full_path, p_name,
		p_action_pass, p_actions, p_view_pass, p_views,
		p_code, p_db_schema, p_db_migration
	);
end;
$$ language plpgsql;



-- call votebase_catalog.insert_ruleset(
-- 	null, 'root',
-- 	'pass', array['root_action'],
-- 	'pass', array['root_view'],
-- 	'', '', ''
-- );
-- select * from votebase_catalog.ruleset;

-- call votebase_catalog.insert_ruleset(
-- 	'root', 'child',
-- 	'pass', array['child_action'],
-- 	'pass', array['child_view'],
-- 	'', '', ''
-- );
-- select * from votebase_catalog.ruleset;





-- drop role if exists votebase_server;
-- drop role if exists ruleset__root__migrator;


-- create role votebase_server createrole noinherit login password 'votebase_server_password';
-- create schema votebase_server;
-- grant CREATE ON DATABASE dev_db to votebase_server;

-- -- when a ruleset is created, we create the schema and base roles used to manage it
-- -- so here we're in the `instate_ruleset` function
-- set role votebase_server;

-- create schema ruleset__root;

-- -- TODO allow createrole on ruleset__root__migrator?
-- create role ruleset__root__migrator noinherit nologin;
-- alter role ruleset__root__migrator set search_path to ruleset__root;
-- grant ALL ON SCHEMA ruleset__root to ruleset__root__migrator;


-- create role ruleset__root__actor noinherit nologin;
-- alter role ruleset__root__actor set search_path to ruleset__root;
-- grant USAGE ON SCHEMA ruleset__root to ruleset__root__actor;

-- -- now we're in the `migrate_ruleset` function
-- -- this is run when the ruleset is initially created at all as well, with whatever the initial is
-- set role ruleset__root__migrator;

-- create table happiness_report ();

-- -- TODO create a sub-role, one that inherits from ruleset__root__actor, and has special privileges


-- -- now we're in the either `execute_view` or `execute_action` functions
-- set role ruleset__root__actor;

-- select * from happiness_report;







-- we probably have to accept that the rulesets have to define *all* their own security rules!
-- the best we can do is try to restrict them based on search path and stuff


-- -- intentionally failing commands >>
-- drop schema votebase_server;
-- create table votebase_server.something();
-- -- intentionally failing commands >>





-- there's the `votebase` role, that is the admin of the server. it's the one that manages the ruleset tables
-- there's a `ruleset migrator` role created just for that purpose, of running migrations for a newly instated ruleset schema. this role used whenever the ruleset is replaced (a ruleset keeps the same name, you can think of the name as the "socket" in the parent ruleset that allows for this ruleset to exist). this role has the rights to modify any object in the schema, including create roles, but *only* in the schema
-- there's a `ruleset` role that the ruleset must inherit from. this is enforced by the fact that only these




-- -- for every ruleset, there will be a schema
-- -- this schema has all the tables for the ruleset
-- -- there has to be a *default* role that's allowed to do things on behalf of that schema
-- -- then any roles created inside that (by the ruleset's migration function) should inherit from it?


-- -- there is the single "votebase" level role that actually has the power to modify database objects, which means it can create ruleset schemas and run the migrations that populate them
-- -- just kidding, there needs to be a separate "migrator" role that does this, since otherwise the migrations could hijack everything
-- -- I think it makes sense to have a default role that can read and write all tables in the ruleset schema
-- -- if the ruleset needs other special roles, it needs to create them and


-- -- perhaps use conf file instead? https://www.postgresql.org/docs/current/config-setting.html#CONFIG-SETTING-CONFIGURATION-FILE
-- alter role all set search_path = "$user"
-- GRANT USAGE ON SCHEMA public TO dashibase_role;

-- create schema authorization schema_name;


-- -- insert into ruleset ("name", parent_name, fn_code, action_names, view_names, db_schema, db_migration) values
-- insert into ruleset ("name", parent_name, fn_code, db_schema, db_migration) values
-- ('root', null, $$votebase.reg("yo", () => { console.log("yo") })$$, $$$$);
-- create schema rulesets__root;
