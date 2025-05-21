create type proposal_status as enum('ACTIVE', 'PASSED', 'REJECTED');

create table proposal (
	id uuid not null default gen_random_uuid(),
	proposed_time timestamptz not null default current_timestamp,
	description text not null,
	proposer_id uuid not null references votebase_catalog.member(id),
	status proposal_status not null default 'ACTIVE'
);

-- create function proposal_is_active(proposal)
-- returns bool as $$
-- 	select current_timestamp < ($1.proposed_time + interval '3 days') ;
-- $$ language sql;

create table vote (
	proposal_id uuid not null references proposal(id),
	member_id uuid not null references votebase_catalog.member(id),
	is_yes bool not null,
	vote_time timestamptz not null default current_timestamp,
	unique (member_id, candidate_id)
);

create function update_proposal_status(proposal_id uuid) returns void as $$
declare
	new_status proposal_status;
	proposed_time timestamptz;
begin
	select proposed_time into proposed_time
	from proposal
	where id = proposal_id and status = 'ACTIVE';

	if proposed_time is null then
		return;
	end if;

	with
	aggregated_proposal as (
		select
			count(*) filter (where is_yes) as yes_count,
			count(*) filter (where not is_yes) as no_count,

			count(*) filter (where is_yes and vote_time < (proposed_time + interval '3 hours')) as before_3_hour_yes_count,
			count(*) filter (where not is_yes and < (proposed_time + interval '3 hours')) as before_3_hour_no_count,


			-- having count(*) filter (where is_yes) >= 3 and count(*) filter (where not is_yes) = 0
			-- if not after_3_hours and yes_count >= 3 and no_count <= 1
		from vote
		where vote.proposal_id = proposal_id and vote_time < (proposed_time + interval '3 days')
	)
	select
		case
			when before_3_hour_yes_count >= 3 and before_3_hour_no_count <= 1 then 'PASSED'

			when current_timestamp < (proposed_time + interval '3 hours') then
				case when no_count > 1 then 'REJECTED' default 'ACTIVE' end

			when current_timestamp < (proposed_time + interval '3 days') then
				case when no_count > 0 then 'REJECTED' default 'ACTIVE' end

			when current_timestamp < (proposed_time + interval '3 days') and no_count = 0 then 'ACTIVE'
			when yes_count >= 1 and no_count = 0 then 'PASSED'
			when no_count > 0 then 'REJECTED'
			default 'ACTIVE'
		end into new_status
	from aggregated_proposal;

	update proposal set status = new_status
	where id = proposal_id;
end;
$$ language plpgsql;


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




-- insert into member (id, "name") values
-- 	(gen_random_uuid(), 'alice'),
-- 	(gen_random_uuid(), 'bob'),
-- 	(gen_random_uuid(), 'carol');

-- insert into candidate_constitution (id, nominator_id, "text")
-- select
-- 	gen_random_uuid(),
-- 	(select id from member where "name" = 'alice'),
-- 	'this is a test constitution';

-- select vote_yes_or_no((member.id, candidate.id, true)::yes_or_no)
-- from member
-- cross join candidate_constitution as candidate
-- where member."name" = 'alice';

-- select * from text_constitution;


-- select vote_yes_or_no((member.id, candidate.id, true)::yes_or_no)
-- from member
-- cross join candidate_constitution as candidate
-- where member."name" = 'bob';

-- select * from text_constitution;


-- select vote_yes_or_no((member.id, candidate.id, true)::yes_or_no)
-- from member
-- cross join candidate_constitution as candidate
-- where member."name" = 'carol';

-- select * from text_constitution;
