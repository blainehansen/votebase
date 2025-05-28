import 'votebase'
import queries from './queries'

votebase.Action('makeProposal', async (description: string, memberId) => {
	if (!memberId) return

	const proposalId = await queries.createProposal(description, memberId)

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

const updateProposalStatus = votebase.Action('updateProposalStatus', async (proposalId: string) => {
	await queries.updateProposalStatus(proposalId)
})


votebase.Action('vote', async ({ isYes, proposalId }: { isYes: boolean, proposalId: string }, memberId) => {
	if (!memberId) return

	await queries.vote(isYes, proposalId, memberId)
})

votebase.View('allProposals', async () => {
	const proposals = await queries.allProposals.fetchAll()

	const renderedProposals = proposals.map(({ proposed_time, description, proposer_id, status }) => {
		const inner = `<p>${description}</p><span>${status}</span><span>(by ${proposer_id})</span>`

		return `<li>${inner}</li>`
	}).join('')

	return `<ul>${renderedProposals}</ul>`
})
