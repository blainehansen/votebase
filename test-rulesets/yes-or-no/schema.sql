create table vote (
	member_id uuid primary key references votebase_catalog.member(id),
	is_yes bool not null,
	message text
);
