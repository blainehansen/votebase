import 'votebase'

type Candidate = Parameters<typeof votebase.proposeSelfReplacement>[0]

votebase.registerAction('seed', async (candidate: Candidate) => {
	const replacementUuid = await votebase.proposeSelfReplacement(candidate)
	// return it to replace self with this candidate
	return replacementUuid
})
