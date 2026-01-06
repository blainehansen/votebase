-- TODO in the future the usable functions and columns will be determined by the "exposes" system of the rulesets, so we'll join or filter by some passed list of relevant uses
-- https://www.postgresql.org/docs/current/catalog-pg-proc.html
--! get_usable_functions
select
	votebase_catalog.unformat_schema_name(sch.nspname) as ruleset_path,
	pgfn.proname::text as function_name,
	pg_catalog.format_type(pgfn.prorettype, null) as return_type,
	case pgfn.provolatile
		when 'i' then false -- 'immutable'
		when 's' then false -- 'stable'
		when 'v' then true -- 'volatile'
	end as is_action,
	votebase_catalog.format_fn_args(pgfn.proargtypes) as function_params

from
	pg_proc as pgfn
	join pg_namespace as sch on pgfn.pronamespace = sch.oid
where
	-- only "normal" functions (no out/inout/variadic)
	pgfn.proargmodes is null
	and sch.nspname not in ('votebase_catalog', 'pg_catalog', 'information_schema')
;


--! get_usable_columns
select
	votebase_catalog.unformat_schema_name(sch.nspname) as ruleset_path,
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


-- --! get_function_grants (:path_prefix?)
--! get_function_grants
select
	regexp_replace(using_role.rolname, '^role:|[|]view$|[|]action$|[|]migrator$', '', 'g') as using_ruleset_path,
	votebase_catalog.unformat_schema_name(used_sch.nspname) as used_ruleset_path,
	used_fn.proname::text as function_name,
	votebase_catalog.format_fn_args(used_fn.proargtypes) as function_params,
	bool_or(acl.privilege_type = 'EXECUTE') as has_execute_priv
from pg_catalog.pg_proc as used_fn
join pg_catalog.pg_namespace as used_sch on used_fn.pronamespace = used_sch.oid
-- TODO I have a suspicion that the acldefault isn't necessary
-- aclexplode(coalesce(used_fn.proacl, acldefault('f', used_fn.proowner))) as acl
cross join lateral aclexplode(used_fn.proacl) as acl
join pg_catalog.pg_roles as using_role on acl.grantee = using_role.oid
where
	acl.privilege_type = 'EXECUTE'
	-- only "normal" functions (no out/inout/variadic)
	and used_fn.proargmodes is null
	and starts_with(using_role.rolname, 'role:')
	-- and starts_with(using_role.rolname, concat('role:', :path_prefix))
group by using_role.rolname, used_sch.nspname, used_fn.proname, used_fn.proargtypes;


-- --! get_column_grants (:path_prefix?)
--! get_column_grants
select
	regexp_replace(using_role.rolname, '^role:|[|]view$|[|]action$|[|]migrator$', '', 'g') as using_ruleset_path,
	votebase_catalog.unformat_schema_name(used_sch.nspname) as used_ruleset_path,
	used_tab.relname::text as table_name,
	used_col.attname::text as column_name,
	bool_or(acl.privilege_type = 'REFERENCES') as has_reference_priv,
	bool_or(acl.privilege_type = 'SELECT') as has_select_priv

from pg_catalog.pg_attribute as used_col
join pg_catalog.pg_class as used_tab on used_col.attrelid = used_tab.oid
join pg_catalog.pg_namespace as used_sch on used_tab.relnamespace = used_sch.oid

cross join lateral aclexplode(used_col.attacl) as acl
join pg_catalog.pg_roles as using_role on acl.grantee = using_role.oid

where
	acl.privilege_type in ('REFERENCES', 'SELECT')
	-- TODO is this check necessary?
	and used_col.attacl is not null
	and used_col.attnum > 0 and not used_col.attisdropped
	and starts_with(using_role.rolname, 'role:')
	and (starts_with(used_sch.nspname, 'ruleset:') or used_sch.nspname = 'votebase_catalog')
	-- and starts_with(using_role.rolname, concat('role:', :path_prefix))
group by using_role.rolname, used_sch.nspname, used_tab.relname, used_col.attname;


--! get_foreign_keys
select
	votebase_catalog.unformat_schema_name(using_sch.nspname) as using_ruleset_path,
	votebase_catalog.unformat_schema_name(used_sch.nspname) as used_ruleset_path,
	used_tab.relname::text as table_name,
	used_col.attname::text as column_name

from pg_catalog.pg_constraint as con
cross join lateral unnest(con.confkey) as ref_col(attnum)
join pg_catalog.pg_attribute as used_col on ref_col.attnum = used_col.attnum

join pg_catalog.pg_class as using_tab on con.conrelid = using_tab.oid
join pg_catalog.pg_namespace as using_sch on using_tab.relnamespace = using_sch.oid

join pg_catalog.pg_class as used_tab on con.confrelid = used_tab.oid
join pg_catalog.pg_namespace as used_sch on used_tab.relnamespace = used_sch.oid

where
	con.contype = 'f'
	and used_col.attnum > 0 and not used_col.attisdropped
	and starts_with(using_sch.nspname, 'ruleset:')
;
