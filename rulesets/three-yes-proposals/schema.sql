create type proposal_status as enum('ACTIVE', 'ACCEPTED', 'REJECTED');

create table proposal (
	id uuid primary key default gen_random_uuid(),
	proposed_time timestamptz not null default current_timestamp,
	description text not null,
	proposer_id uuid not null references votebase_catalog.member(id),
	status proposal_status not null default 'ACTIVE'
);

create table proposal_vote (
	proposal_id uuid not null references proposal(id),
	member_id uuid not null references votebase_catalog.member(id),
	is_yes bool not null,
	vote_time timestamptz not null default current_timestamp,
	primary key (proposal_id, member_id)
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
		from proposal_vote
		where proposal_vote.proposal_id = proposal_id
	)
	select
		case
			-- If we're before 3 hours, then if there are 2 or more downvotes, the proposal is rejected early.
			when current_timestamp < (proposed_time + interval '3 hours') then
				case when no_count >= 2 then 'REJECTED' else 'ACTIVE' end

			-- If we're after 3 hours, then
			when current_timestamp < (proposed_time + interval '3 days') then case
				-- if there are at least 3 upvotes and at most 1 downvote, the proposal is accepted early.
				when yes_count >= 3 and no_count <= 1 then 'ACCEPTED'
				-- if there is 2 or more downvotes, the proposal is rejected early.
				when no_count >= 2 then 'REJECTED'
				else 'ACTIVE'
			end

			-- If we're after 3 days, then finally the proposal is accepted if there is at least 1 upvote and 0 downvotes, and rejected otherwise
			else
				case when yes_count >= 1 and no_count = 0 then 'ACCEPTED' else 'REJECTED' end
		end into new_status
	from aggregated_proposal;

	update proposal set status = new_status
	where id = proposal_id;
end;
$$ language plpgsql;

create function cast_vote(p_proposal_id uuid, p_member_id uuid, p_is_yes bool) returns void as $$
	insert into proposal_vote (proposal_id, member_id, is_yes)
	values (p_proposal_id, p_member_id, p_is_yes)
	on conflict (proposal_id, member_id)
	do update set is_yes = excluded.is_yes, vote_time = current_timestamp;

	select update_proposal_status(p_proposal_id);
$$ language sql;


-- insert into member (id, "name") values
-- 	(gen_random_uuid(), 'alice'),
-- 	(gen_random_uuid(), 'bob'),
-- 	(gen_random_uuid(), 'carol');
