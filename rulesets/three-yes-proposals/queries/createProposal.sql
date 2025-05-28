insert into proposal (description, proposer_id)
values (:description, :proposer_id)
returning id;
