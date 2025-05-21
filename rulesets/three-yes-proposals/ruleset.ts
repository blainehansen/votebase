import 'votebase'
import queries from './queries'

votebase.Action('makeProposal', async (description: string, userId) => {
	if (!userId) return

	const proposalId = await queries.createProposal(description, userId)

	const now = new Date()
	const threeHours = new Date(now)
	threeHours.setHours(threeHours.getHours() + 3)
	const threeDays = new Date(now)
	threeDays.setDate(threeDays.getDate() + 3)

	await Promise.all([
		votebase.scheduleAction(`3 hour check for proposal ${proposalId} (${description})`, threeHours, updateProposalStatus, proposalId),
		votebase.scheduleAction(`3 day check for proposal ${proposalId} (${description})`, threeDays, updateProposalStatus, proposalId),
	])
})

const updateProposalStatus = votebase.Action('updateProposalStatus', async (proposalId: string, userId) => {
	await queries.updateProposalStatus(proposalId, userId)

})


votebase.Action('vote', async ({ isYes, proposalId }: { isYes: boolean, proposalId: string }, userId) => {
	if (!userId) return

	await queries.vote(isYes, proposalId, userId)
})

votebase.View('allProposals', async () => {
	const proposals = await queries.allProposals.fetchAll()

	const renderedProposals = proposals.map(({ proposed_time, description, proposer_id, status }) => {
		const inner = `<p>${description}</p><span>${status}</span>`

		return `<li>${inner}</li>`
	}).join('')

	return `<ul>${renderedProposals}</ul>`
})
