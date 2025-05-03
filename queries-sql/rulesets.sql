--! insert_ruleset (parent_full_path?)
insert into votebase_catalog.ruleset (
	parent_full_path, "name", actions, views, code, db_schema
) values (
	:parent_full_path, :name, :actions, :views, :code, :db_schema
) returning full_path, migrator_pass, action_pass, view_pass;

--! insert_candidate_replacement
select u as candidate_uuid
from votebase_catalog.insert_candidate_replacement(
	:full_path, :actions, :views, :code, :db_schema, :db_migration
) as t(u);

--! apply_candidate
select m as "db_migration"
from votebase_catalog.apply_candidate(:candidate_uuid) as t(m);
