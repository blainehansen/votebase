create table member (
	id uuid primary key,
	"name" text not null
);

create table text_constitution (
	dummy_constant bool not null default true check(dummy_constant) unique,
	"text" text not null
);
create unique index single_text_constitution
on text_constitution((true)) where true;

create table candidate_constitution (
	id uuid primary key,
	nominator_id uuid not null references member(id),
	"text" text not null
);

create table yes_or_no (
	member_id uuid not null references member(id),
	candidate_id uuid not null references candidate_constitution(id),
	is_yes bool not null,
	unique (member_id, candidate_id)
);

create or replace function vote_yes_or_no(vote yes_or_no) returns void as $$
declare
	winning_candidate_id uuid;
begin
	insert into yes_or_no (member_id, candidate_id, is_yes)
	values (vote.member_id, vote.candidate_id, vote.is_yes)
	on conflict (member_id, candidate_id)
	do update set is_yes = excluded.is_yes;

	select candidate_id into winning_candidate_id
	from yes_or_no
	where candidate_id = vote.candidate_id
	group by candidate_id
	having count(*) filter (where is_yes) >= 3 and count(*) filter (where not is_yes) = 0;

	if winning_candidate_id is null then
		return;
	end if;

	delete from text_constitution;
	insert into text_constitution ("text")
	select "text" from candidate_constitution where id = winning_candidate_id;

	delete from yes_or_no;
	delete from candidate_constitution;
end;
$$ language plpgsql;




insert into member (id, "name") values
	(gen_random_uuid(), 'alice'),
	(gen_random_uuid(), 'bob'),
	(gen_random_uuid(), 'carol');

insert into candidate_constitution (id, nominator_id, "text")
select
	gen_random_uuid(),
	(select id from member where "name" = 'alice'),
	'this is a test constitution';

select vote_yes_or_no((member.id, candidate.id, true)::yes_or_no)
from member
cross join candidate_constitution as candidate
where member."name" = 'alice';

select * from text_constitution;


select vote_yes_or_no((member.id, candidate.id, true)::yes_or_no)
from member
cross join candidate_constitution as candidate
where member."name" = 'bob';

select * from text_constitution;


select vote_yes_or_no((member.id, candidate.id, true)::yes_or_no)
from member
cross join candidate_constitution as candidate
where member."name" = 'carol';

select * from text_constitution;
