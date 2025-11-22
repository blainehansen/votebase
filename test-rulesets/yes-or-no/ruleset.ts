import queries from './queries'

export type YesOrNo = { isYes: boolean, message?: string | null }
votebase.Action('voteYesOrNo', async ({ isYes, message }: YesOrNo, memberId) => {
	if (!memberId) throw new Error("voteYesOrNo can't be called without being logged in!")

	await queries.voteYesOrNo(memberId, isYes, message ?? null)
})

votebase.View('currentCount', async () => {
	return JSON.stringify(await queries.currentCount.fetchAll())
})

votebase.View('anonymousVotes', async () => {
	return JSON.stringify(await queries.anonymousVotes.fetchAll())
})
