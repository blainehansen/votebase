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
	parent_full_path, "name", actions, views, code, db_schema, db_uses_functions, db_uses_tables
) values (
	:parent_full_path, :name, :actions, :views, :code, :db_schema, :db_uses_functions, :db_uses_tables
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
select full_path as using_full_path, db_uses_functions, db_uses_tables
from votebase_catalog.ruleset;
-- where full_path in (:db_uses.ruleset);


-- select
-- 	sch.nspname::text as schema_name,
-- 	pgfn.proname::text as function_name,
-- 	pg_catalog.pg_get_function_identity_arguments(pgfn.oid) as params,
-- 	pg_catalog.format_type(pgfn.prorettype, null) as return_type,
-- 	case pgfn.provolatile
-- 		when 'i' then false -- 'immutable'
-- 		when 's' then false -- 'stable'
-- 		when 'v' then true -- 'volatile'
-- 	end as is_action
-- from
-- 	pg_catalog.pg_proc as pgfn
-- left join
-- 	pg_catalog.pg_namespace as sch on sch.oid = pgfn.pronamespace
-- where
-- 	-- TODO need to also exclude all the non-usable functions in votebase_catalog?
-- 	sch.nspname not in ('pg_catalog', 'information_schema')
-- 	-- 'f' for function, 'p' for procedure, 'a' for aggregate, 'w' for window function
-- 	and pgfn.prokind in ('f', 'p')
-- ;


-- https://www.postgresql.org/docs/current/catalog-pg-proc.html
--! get_usable_functions
select
	sch.nspname::text as schema_name,
	pgfn.proname::text as function_name,
	pg_catalog.format_type(pgfn.prorettype, null) as return_type,
	case pgfn.provolatile
		when 'i' then false -- 'immutable'
		when 's' then false -- 'stable'
		when 'v' then true -- 'volatile'
	end as is_action,

	array(
		select
			row(
				coalesce(nullif(p.arg_name, ''), '<unnamed positional>'),
				pg_catalog.format_type(p.arg_type_oid, null)
			)::votebase_catalog.rough_param_struct
		from
			-- "zip" the two arrays?
			unnest(
				pgfn.proargtypes::oid[],
				-- handle cases where proargnames is null by filling with nulls of correct length
				coalesce(pgfn.proargnames, array_fill(null::text, array[pgfn.pronargs]))
			)
			-- including ordinality to ensure order stays?
			with ordinality as p(arg_type_oid, arg_name, ordinality)
	) as params

from
	pg_proc as pgfn
	join pg_namespace as sch on pgfn.pronamespace = sch.oid
where
	-- only "normal" functions (no out/inout/variadic)
	pgfn.proargmodes is null
	-- TODO also need to exclude votebase_catalog?
	and sch.nspname not in ('votebase_catalog', 'pg_catalog', 'information_schema')
;

--! get_usable_columns
select
	sch.nspname::text as schema_name,
	tab.relname::text as table_name,
	array_agg(
		row(
			col.attname::text,
			pg_catalog.format_type(col.atttypid, null),
			col.attnotnull
			-- pg_get_expr(col_detail.adbin, col_detail.adrelid) is not null as has_default
			-- pg_get_expr(col_detail.adbin, col_detail.adrelid) as default_value
		)::votebase_catalog.rough_column_struct
	) as columns
from
	pg_catalog.pg_attribute as col
	join pg_catalog.pg_class as tab on col.attrelid = tab.oid
	join pg_catalog.pg_namespace as sch on tab.relnamespace = sch.oid
	-- left join pg_catalog.pg_attrdef as col_detail on (col.attrelid = col_detail.adrelid and col.attnum = col_detail.adnum)
where
	tab.relkind in ('r', 'v', 'm')
	and col.attnum > 0 -- no system columns
	and not col.attisdropped
	and sch.nspname not in ('pg_catalog', 'information_schema')
	and not (sch.nspname = 'votebase_catalog' and tab.relname != 'member' and col.attname != 'id')
group by sch.nspname, tab.relname;
