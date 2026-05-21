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
select ts_code
from votebase_catalog.ruleset
where full_path = :ruleset_full_path and votebase_catalog.has_fn(fns, :view_name, 'View');

--! get_action_details
select ts_code
from votebase_catalog.ruleset
where full_path = :ruleset_full_path and votebase_catalog.has_fn(fns, :action_name, 'Action');

-- TODO https://dba.stackexchange.com/questions/195603/create-json-object-from-recursive-tree-structure
--! get_stored_ruleset
select ts_code, db_schema
from votebase_catalog.ruleset
where full_path = :ruleset_full_path;

--! get_stored_schemas
select full_path, /*parent_full_path, full_path,*/ db_schema
from votebase_catalog.ruleset;

-- MODIFYING RULESETS

--! insert_ruleset (parent_full_path?)
insert into votebase_catalog.ruleset (
	parent_full_path, "name",
	ts_code, db_schema, fns
) values (
	:parent_full_path, :name,
	:ts_code, :db_schema, :fns
) returning full_path;

--! delete_ruleset
delete from votebase_catalog.ruleset
where full_path = :full_path;

--! update_ruleset
update votebase_catalog.ruleset set
	ts_code = :ts_code, db_schema = :db_schema, fns = :fns
where full_path = :full_path;

-- MODIFYING RULESET CANDIDATES

--! insert_candidate_replacement
insert into votebase_catalog.candidate_replacement_ruleset (candidate_for, ts_code, db_schema, db_migration, fns)
values (:candidate_for, :ts_code, :db_schema, :db_migration, :fns)
returning id;

--! get_ruleset_candidate
select candidate_for, ts_code, db_schema, db_migration, fns
from votebase_catalog.candidate_replacement_ruleset
where id = :candidate_id;

--! delete_candidate_only
delete from votebase_catalog.candidate_replacement_ruleset
where id = :candidate_id;

--! delete_candidate_and_others
with
target_candidate as (
	select candidate_for, ts_code, db_schema, db_migration, fns
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

-- TESTING FUNCTIONS

--! test_select_candidate_replacement_ruleset
select id, candidate_for, ts_code, db_schema, db_migration, fns
from votebase_catalog.candidate_replacement_ruleset;

--! test_get_ruleset : (parent_full_path?)
select *
from votebase_catalog.ruleset
where full_path = :full_path;
