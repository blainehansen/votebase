create schema a
	create table a(id integer primary key);

create schema b
	create table b(i integer references a.a(id));

SELECT
	conrelid::regclass AS table_name,
	conname AS foreign_key,
	pg_get_constraintdef(oid) AS definition
FROM
	pg_constraint
WHERE
	contype = 'f' -- 'f' denotes a foreign key constraint
;



drop schema a cascade;


SELECT
	conrelid::regclass AS table_name,
	conname AS foreign_key,
	pg_get_constraintdef(oid) AS definition
FROM
	pg_constraint
WHERE
	contype = 'f' -- 'f' denotes a foreign key constraint
;
