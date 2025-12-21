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


--! get_ruleset_migrator
select migrator_pass
from votebase_catalog.ruleset
where full_path = :ruleset_full_path;

--! insert_ruleset (parent_full_path?)
insert into votebase_catalog.ruleset (
	parent_full_path, "name", actions, views, code, db_schema
) values (
	:parent_full_path, :name, :actions, :views, :code, :db_schema
) returning full_path, migrator_pass, action_pass, view_pass;

--! delete_ruleset
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



--! get_possibly_effected_uses
select full_path as using_full_path, db_uses
from votebase_catalog.ruleset;
-- where full_path in (:db_uses.ruleset);

--! get_usable_functions
select
	sch.nspname as schema_name,
	pgfn.proname as function_name,
	pg_catalog.pg_get_function_identity_arguments(pgfn.oid) as arguments,
	pg_catalog.format_type(pgfn.prorettype, null) as return_type,
	case pgfn.provolatile
		when 'i' then false -- 'immutable'
		when 's' then false -- 'stable'
		when 'v' then true -- 'volatile'
	end as is_action
from
	pg_catalog.pg_proc as pgfn
left join
	pg_catalog.pg_namespace as sch on sch.oid = pgfn.pronamespace
where
	sch.nspname not in ('pg_catalog', 'information_schema')
	-- 'f' for function, 'p' for procedure, 'a' for aggregate, 'w' for window function
	and pgfn.prokind in ('f', 'p')
;


--! get_usable_columns
select
-- sch.nspname as schema_name,
-- tab.relname as table_name,
tab.oid as table_oid,
-- col.attname as column_name,
col.attnum as column_id,
col.attnotnull as not_null
-- pg_get_expr(col_detail.adbin, col_detail.adrelid) is not null as has_default
-- pg_get_expr(col_detail.adbin, col_detail.adrelid) as default_value
from
pg_catalog.pg_attribute as col
join pg_catalog.pg_class as tab on col.attrelid = tab.oid
join pg_catalog.pg_namespace as sch on tab.relnamespace = sch.oid
left join pg_catalog.pg_attrdef as col_detail on (col.attrelid = col_detail.adrelid and col.attnum = col_detail.adnum)
where
tab.relkind in ('r', 'v', 'm')
and col.attnum > 0 -- no system columns
and not col.attisdropped -- no dropped columns
and sch.nspname not in ('pg_catalog', 'information_schema');

-- select
-- 	-- sch.nspname as schema_name,
-- 	-- tab.relname as table_name,
-- 	tab.oid as table_oid,
-- 	-- col.attname as column_name,
-- 	col.attnum as column_id,
-- 	col.attnotnull as not_null
-- 	-- pg_get_expr(col_detail.adbin, col_detail.adrelid) is not null as has_default
-- 	-- pg_get_expr(col_detail.adbin, col_detail.adrelid) as default_value
-- from
-- 	pg_catalog.pg_attribute as col
-- 	join pg_catalog.pg_class as tab on col.attrelid = tab.oid
-- 	join pg_catalog.pg_namespace as sch on tab.relnamespace = sch.oid
-- 	left join pg_catalog.pg_attrdef as col_detail on (col.attrelid = col_detail.adrelid and col.attnum = col_detail.adnum)
-- where
-- 	tab.relkind in ('r', 'v', 'm')
-- 	and col.attnum > 0 -- no system columns
-- 	and not col.attisdropped -- no dropped columns
-- 	and sch.nspname not in ('pg_catalog', 'information_schema');
