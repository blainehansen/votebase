/*
	@name Nominate
*/
insert into candidate_constitution (nominator_id, "text")
values (:nominator_id!, :candidate_text!)
returning id;

/*
	@name VoteYesOrNo
*/
select vote_yes_or_no((:member_id!, :candidate_id!, :yes_or_no!)::yes_or_no);

--  npm install --save-dev @pgtyped/cli @pgtyped/query typescript

--  npx pgtyped -c pgtyped.config.json
