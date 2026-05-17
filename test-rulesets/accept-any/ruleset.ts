import type { BundledRuleset } from 'votebase'

votebase.Action('replaceSelf', async (candidate: BundledRuleset) => {
	const replacementUuid = await votebase.proposeSelfReplacement(candidate)
	console.log('replacementUuid:', replacementUuid)
	return { replace_self_with_uuid: replacementUuid, delete_other_candidates: true }
})
