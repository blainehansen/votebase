-- FETCHING RULESET INFO

--! get_rulesets
select full_path
from votebase_catalog.ruleset;

--! get_ruleset_views
select votebase_catalog.filter_fns(fns, 'View')
from votebase_catalog.ruleset
where full_path = :ruleset_full_path;

--! get_ruleset_detail
select votebase_catalog.filter_fns(fns, 'View'), ts_code, db_schema
from votebase_catalog.ruleset
where full_path = :ruleset_full_path;

--! get_view_details
select ts_code, view_pass as pass
from votebase_catalog.ruleset
where full_path = :ruleset_full_path and votebase_catalog.has_fn(fns, :view_name, 'View');

--! get_action_details
select ts_code, action_pass, migrator_pass
from votebase_catalog.ruleset
where full_path = :ruleset_full_path and votebase_catalog.has_fn(fns, :action_name, 'Action');

--! get_ruleset_migrator
select migrator_pass
from votebase_catalog.ruleset
where full_path = :ruleset_full_path;

-- https://dba.stackexchange.com/questions/195603/create-json-object-from-recursive-tree-structure

-- MODIFYING RULESETS

--! insert_ruleset (parent_full_path?)
insert into votebase_catalog.ruleset (
	parent_full_path, "name",
	ts_code, db_schema, fns, db_uses_functions, db_uses_tables
) values (
	:parent_full_path, :name,
	:ts_code, :db_schema, :fns, :db_uses_functions, :db_uses_tables
) returning full_path, migrator_pass, action_pass, view_pass;

--! delete_ruleset
delete from votebase_catalog.ruleset
where full_path = :full_path;

--! update_ruleset
update votebase_catalog.ruleset set
	ts_code = :ts_code, db_schema = :db_schema, fns = :fns,
	db_uses_functions = :db_uses_functions, db_uses_tables = :db_uses_tables
where full_path = :full_path;

-- MODIFYING RULESET CANDIDATES

--! insert_candidate_replacement
insert into votebase_catalog.candidate_replacement_ruleset (candidate_for, bundled_ruleset)
values (:candidate_for, :bundled_ruleset)
returning id;

--! get_ruleset_candidate
select candidate_for, bundled_ruleset
from votebase_catalog.candidate_replacement_ruleset
where id = :candidate_id;

--! delete_candidate_only
delete from votebase_catalog.candidate_replacement_ruleset
where id = :candidate_id;

--! delete_candidate_and_others
with
target_candidate as (
	select candidate_for, bundled_ruleset
	from votebase_catalog.candidate_replacement_ruleset
	where id = :candidate_id
)
delete from votebase_catalog.candidate_replacement_ruleset as ruleset
using target_candidate
where ruleset.candidate_for = target_candidate.candidate_for;

-- --! take_candidate_deleting_others
-- with
-- target_candidate as (
-- 	select candidate_for, bundled_ruleset
-- 	from votebase_catalog.candidate_replacement_ruleset
-- 	where id = :candidate_id
-- ),
-- candidate_deletions as (
-- 	delete from votebase_catalog.candidate_replacement_ruleset as ruleset
-- 	using target_candidate
-- 	where ruleset.candidate_for = target_candidate.candidate_for
-- )
-- select candidate_for, bundled_ruleset from target_candidate;

-- --! take_candidate_not_deleting_others
-- delete from votebase_catalog.candidate_replacement_ruleset
-- where id = :candidate_uuid
-- returning candidate_for, bundled_ruleset;

-- DB USES THINGS

--! get_possibly_effected_uses
select full_path as using_full_path, db_uses_functions, db_uses_tables
from votebase_catalog.ruleset;
-- where full_path in (:db_uses.ruleset);

-- -- TESTING FUNCTIONS

--! test_select_candidate_replacement_ruleset
select candidate_for, bundled_ruleset
from votebase_catalog.candidate_replacement_ruleset;

