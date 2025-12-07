create function yep_fn() returns text as $$
	select 'a'
$$ language sql;

create function yo() returns void as $$
	select * from yep_fn();
$$ language sql;

drop function yep_fn;

select * from yo();
