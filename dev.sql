-- create schema a
-- 	create table a(id integer primary key);

-- create schema b
-- 	create table b(i integer references a.a(id));

-- SELECT
-- 	conrelid::regclass AS table_name,
-- 	conname AS foreign_key,
-- 	pg_get_constraintdef(oid) AS definition
-- FROM
-- 	pg_constraint
-- WHERE
-- 	contype = 'f' -- 'f' denotes a foreign key constraint
-- ;



-- drop schema a cascade;


-- SELECT
-- 	conrelid::regclass AS table_name,
-- 	conname AS foreign_key,
-- 	pg_get_constraintdef(oid) AS definition
-- FROM
-- 	pg_constraint
-- WHERE
-- 	contype = 'f' -- 'f' denotes a foreign key constraint
-- ;

-- select grantor, grantee, table_schema, table_name, column_name, privilege_type
-- from information_schema.column_privileges
-- where grantee = 'role:b|view' and table_schema = 'used';





select
	regexp_replace(using_role.rolname, '^role:|[|table_schema]view$|[|]action$|[|]migrator$', '', 'g') as using_schema,
	used_sch.nspname as used_schema,
	used_fn.proname as function_name,
	pg_get_function_identity_arguments(used_fn.oid) as function_args,
	bool_or(acl.privilege_type = 'EXECUTE') as has_execute_priv
from pg_catalog.pg_proc as used_fn
join pg_catalog.pg_namespace as used_sch on used_fn.pronamespace = used_sch.oid
-- TODO I have a suspicion that the acldefault isn't necessary
-- aclexplode(coalesce(used_fn.proacl, acldefault('f', used_fn.proowner))) as acl
cross join lateral aclexplode(used_fn.proacl) as acl
join pg_catalog.pg_roles as using_role on acl.grantee = using_role.oid
where
	acl.privilege_type = 'EXECUTE'
	and used_fn.proargmodes is null
	-- and using_role.rolname = 'role:b|view' and used_sch.nspname = 'used'
	and starts_with(using_role.rolname, 'role:') and used_sch.nspname = 'used'
group by 1, 2, 3, 4;


with
referencing_keys as (
	select
		conrelid as using_tab_oid,
		confrelid as used_tab_oid,
		using_sch.nspname as using_schema,
		used_sch.nspname as used_schema,
		unnest(confkey) as used_col_attnum
	from pg_catalog.pg_constraint

	join pg_catalog.pg_class as using_tab on conrelid = using_tab.oid
	join pg_catalog.pg_namespace as using_sch on using_tab.relnamespace = using_sch.oid

	join pg_catalog.pg_class as used_tab on confrelid = used_tab.oid
	join pg_catalog.pg_namespace as used_sch on used_tab.relnamespace = used_sch.oid

	where
		contype = 'f' -- foreign key constraints
		and (using_sch.nspname = 'used' or used_sch.nspname = 'used')
)
select
	using_role.rolname as using_grantee,
	regexp_replace(using_role.rolname, '^role:|[|]view$|[|]action$|[|]migrator$', '', 'g') as using_schema,
	used_sch.nspname as used_schema,
	used_tab.relname as table_name,
	used_col.attname as column_name,
	ref.used_col_attnum is not null as has_referencing_key,
	bool_or(acl.privilege_type = 'REFERENCES') as has_reference_priv,
	bool_or(acl.privilege_type = 'SELECT') as has_select_priv
from pg_catalog.pg_attribute as used_col
join pg_catalog.pg_class as used_tab on used_col.attrelid = used_tab.oid
join pg_catalog.pg_namespace as used_sch on used_tab.relnamespace = used_sch.oid

cross join lateral aclexplode(used_col.attacl) as acl
join pg_catalog.pg_roles as using_role on acl.grantee = using_role.oid

left join referencing_keys as ref on
	used_sch.nspname = ref.used_schema
	and regexp_replace(using_role.rolname, '^role:|[|]view$|[|]action$|[|]migrator$', '', 'g') = ref.using_schema
	and used_col.attnum = ref.used_col_attnum

where
	acl.privilege_type in ('REFERENCES', 'SELECT')
	and starts_with(using_role.rolname, 'role:') and used_sch.nspname = 'used'
	and used_col.attnum > 0 and not used_col.attisdropped
	and used_col.attacl is not null
group by 1, 2, 3, 4, 5, 6;

