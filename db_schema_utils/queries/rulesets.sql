--! get_rulesets
select full_path
from votebase_catalog.ruleset;

--! get_ruleset_views
select views
from votebase_catalog.ruleset
where full_path = :ruleset_full_path;

--! get_ruleset_detail
select views, code, db_schema
from votebase_catalog.ruleset
where full_path = :ruleset_full_path;

--! get_action_details
select code, action_pass, migrator_pass
from votebase_catalog.ruleset
where full_path = :ruleset_full_path and :action_name = ANY(actions);

--! get_view_details
select code, view_pass as pass
from votebase_catalog.ruleset
where full_path = :ruleset_full_path and :view_name = ANY(views);

--! insert_ruleset (parent_full_path?)
insert into votebase_catalog.ruleset (
	parent_full_path, "name", actions, views, code, db_schema
) values (
	:parent_full_path, :name, :actions, :views, :code, :db_schema
) returning full_path, migrator_pass, action_pass, view_pass;

--! delete_ruleset (full_path)
delete from votebase_catalog.ruleset
where full_path = :full_path;

--! insert_candidate_replacement
select u as candidate_uuid
from votebase_catalog.insert_candidate_replacement(
	:full_path, :actions, :views, :code, :db_schema, :db_migration
) as t(u);

--! apply_candidate
select m as "db_migration"
from votebase_catalog.apply_candidate(:candidate_uuid) as t(m);

--! test_select_candidate_replacement_ruleset
select candidate_for, actions, views, code, db_schema, db_migration
from votebase_catalog.candidate_replacement_ruleset;
