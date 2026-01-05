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
	regexp_replace(rol.rolname, '^role:|[|table_schema]view$|[|]action$|[|]migrator$', '', 'g') as using_schema,
	sch.nspname as used_schema,
	pgfn.proname as function_name,
	pg_get_function_identity_arguments(pgfn.oid) as function_args,
	bool_or(acl.privilege_type = 'EXECUTE') as has_execute_priv
from
	pg_catalog.pg_proc as pgfn
join
	pg_catalog.pg_namespace as sch on pgfn.pronamespace = sch.oid
cross join lateral
	-- TODO I have a suspicion that the acldefault isn't necessary
	-- aclexplode(coalesce(pgfn.proacl, acldefault('f', pgfn.proowner))) as acl
	aclexplode(pgfn.proacl) as acl
join
	pg_catalog.pg_roles as rol on acl.grantee = rol.oid
where
	acl.privilege_type = 'EXECUTE'
	and pgfn.proargmodes is null
	-- and rol.rolname = 'role:b|view' and sch.nspname = 'used'
	and starts_with(rol.rolname, 'role:') and sch.nspname = 'used'
group by 1, 2, 3, 4;



-- TODO this is way more complicated than this
-- you need to have a query that correctly correlates the privilege of the reference, which has a grantee that implies a schema name, with the actual origin of the foreign key

with grantee_privileges as (
	select
		grantee as using_grantee, table_schema as used_schema, table_name, column_name,
		bool_or(privilege_type = 'SELECT') AS has_select_priv,
		bool_or(privilege_type = 'REFERENCES') as has_reference_priv
	from information_schema.column_privileges
	where
		privilege_type in ('SELECT', 'REFERENCES')
		and grantee = 'role:b|view' and table_schema = 'used'
	group by grantee, table_schema, table_name, column_name
),
referenced_columns as (
	select distinct kcu.table_schema, kcu.table_name, kcu.column_name
	from
		information_schema.referential_constraints as rc
		join
			information_schema.key_column_usage as kcu
			on rc.unique_constraint_catalog = kcu.constraint_catalog and rc.unique_constraint_schema = kcu.constraint_schema and rc.unique_constraint_name = kcu.constraint_name
)
select
	regexp_replace(gp.using_grantee, '^role:|[|]view$|[|]action$|[|]migrator$', '', 'g') as using_schema,
	gp.used_schema, gp.table_name, gp.column_name, gp.has_select_priv, gp.has_reference_priv,
	-- true if this specific column is part of a key referenced by a foreign key
	case
		when rc.column_name is not null then true
		else false
	end as has_referencing_key
from
	grantee_privileges gp
	left join referenced_columns as rc on
		gp.used_schema = rc.table_schema
		and gp.table_name = rc.table_name
		and gp.column_name = rc.column_name
;



-- with
-- referencing_keys as (
-- 	select distinct
-- 		confrelid,
-- 		unnest(confkey) as conattnum
-- 	from pg_catalog.pg_constraint
-- 	where contype = 'f' -- foreign key constraints
-- )
-- select
-- 	rol.rolname as using_grantee,
-- 	sch.nspname as used_schema,
-- 	tab.relname as table_name,
-- 	col.attname as column_name,
-- 	rk.attnum is not null as has_referencing_key
-- 	bool_or(eg.priv_type = 'SELECT') as has_select_priv,
-- 	bool_or(eg.priv_type = 'REFERENCES') as has_reference_priv,
-- from pg_catalog.pg_attribute as col
-- join pg_catalog.pg_class as tab on col.attrelid = tab.oid
-- left join referencing_keys as rk on rk.confrelid = tab.id and rk.conattnum = col.attnum
-- join pg_catalog.pg_namespace as sch on tab.relnamespace = sch.oid
-- cross join lateral aclexplode(col.attacl) as acl
-- join pg_catalog.pg_roles as rol on acl.grantee = rol.oid
-- where
-- 	acl.privilege_type in ('SELECT', 'REFERENCES')
-- 	and starts_with(rol.rolname, 'role:') and sch.nspname = 'used'
-- 	col.attnum > 0 and not col.attisdropped
-- 	and col.attacl is not null
-- group by 1, 2, 3, 4, 5;



-- with target_roles as (
-- 	select oid from pg_roles
-- 	where rolname = 'role:b|view'
-- ),
-- referencing_keys as (
-- 	select distinct
-- 		confrelid as relid,
-- 		unnest(confkey) as attnum
-- 	from
-- 		pg_catalog.pg_constraint
-- 	where
-- 		contype = 'f' -- foreign key constraints
-- ),
-- expanded_grants as (
-- 	select
-- 		(aclexplode(a.attacl)).grantee as grantee_oid,
-- 		(aclexplode(a.attacl)).privilege_type as priv_type,
-- 		a.attrelid as relid,
-- 		a.attnum
-- 	from
-- 		pg_catalog.pg_attribute a
-- 	where
-- 		a.attnum > 0 and not a.attisdropped
-- 		and a.attacl is not null
-- ),
-- consolidated_privs as (
-- 	select
-- 		eg.grantee_oid,
-- 		eg.relid,
-- 		eg.attnum,
-- 		bool_or(eg.priv_type = 'SELECT') as has_select_priv,
-- 		bool_or(eg.priv_type = 'REFERENCES') as has_reference_priv
-- 	from
-- 		expanded_grants eg
-- 	where
-- 		eg.priv_type IN ('SELECT', 'REFERENCES')
-- 	group by
-- 		eg.grantee_oid, eg.relid, eg.attnum
-- )
-- select
-- 	r.rolname as using_grantee,
-- 	n.nspname as used_schema,
-- 	c.relname as table_name,
-- 	a.attname as column_name,
-- 	cp.has_select_priv,
-- 	cp.has_reference_priv,
-- 	-- check if this column is referenced by a foreign key
-- 	case
-- 		when rk.attnum is not null then true
-- 		else false
-- 	end as has_referencing_key
-- from
-- 	consolidated_privs cp
-- join
-- 	pg_catalog.pg_class c on cp.relid = c.oid
-- join
-- 	pg_catalog.pg_namespace n on c.relnamespace = n.oid
-- join
-- 	pg_catalog.pg_attribute a on cp.relid = a.attrelid and cp.attnum = a.attnum
-- left join
-- 	pg_catalog.pg_roles r on cp.grantee_oid = r.oid
-- left join
-- 	referencing_keys rk on cp.relid = rk.relid and cp.attnum = rk.attnum
-- where
-- 	-- apply the role filter if specified in the cte, or remove this check for all
-- 	(exists (select 1 from target_roles where oid = cp.grantee_oid) or cp.grantee_oid = 0)
-- order by
-- 	using_grantee, used_schema, table_name, column_name;
