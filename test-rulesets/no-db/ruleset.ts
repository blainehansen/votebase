// this ruleset is fundamentally flawed, because the ephemeral counter will be reset on every run!
// this is the sort of thing I'd love to prevent in the future using a flow effects system
let ephemeralCounter = 0

votebase.View('seeCounter', () => {
	return `${ephemeralCounter}`
})

votebase.Action('incCounter', () => {
	ephemeralCounter += 1
})

votebase.Action('decCounter', () => {
	if (ephemeralCounter <= 0) ephemeralCounter = 0
	else ephemeralCounter -= 1
})
