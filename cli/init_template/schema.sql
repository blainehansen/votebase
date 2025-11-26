-- docs for how to write a ruleset schema:
-- TODO blaine

create table fruit (
	id uuid primary key,
	name text not null,
	inserted_by_member_id uuid not null references votebase_catalog.member(id)
);
