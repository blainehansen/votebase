export default {
	"voteYesOrNo": __.StatementExecutor(`INSERT INTO vote (member_id, is_yes, message) VALUES ($1, $2, $3) ON CONFLICT(member_id) DO UPDATE SET is_yes = excluded.is_yes, message = excluded.message`, ['Text', 'Bool', 'Text?'] as [member_id: 'Text', is_yes: 'Bool', message: 'Text?']),
	"anonymousVotes": new __.QueryExecutor(`SELECT is_yes, message FROM vote ORDER BY is_yes`, [] as [], [[`is_yes`, 'Bool'], [`message`, 'Text?']] as const),
	"currentCount": new __.QueryExecutor(`SELECT count(CASE WHEN is_yes THEN 1 ELSE NULL END) AS yes_count, count(CASE WHEN NOT is_yes THEN 1 ELSE NULL END) AS no_count FROM vote`, [] as [], [[`yes_count`, 'I64?'], [`no_count`, 'I64?']] as const),
}
