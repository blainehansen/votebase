type Candidate = Parameters<typeof votebase.proposeSelfReplacement>[0]

votebase.registerAction('__insert_initial', async (candidate: Candidate) => {
	const replacementUuid = await votebase.proposeSelfReplacement(candidate)
	console.log('replacementUuid:', replacementUuid)
	// return it to replace self with this candidate
	return replacementUuid
})
