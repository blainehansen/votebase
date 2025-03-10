import * as z from 'zod'
import { Async } from './index'
export { displayError } from './index'

// TODO get proper server prefix
// https://nuxt.com/docs/guide/going-further/runtime-config#environment-variables
const SERVER_PREFIX = 'http://localhost:8080'

export const RulesetListing = z.strictObject({
	full_path: z.string(),
})
export type RulesetListing = z.infer<typeof RulesetListing>

export function asyncRulesets() {
	return Async.fetch(z.array(RulesetListing), SERVER_PREFIX + '/rulesets')
}


export const Ruleset = z.strictObject({
	full_path: z.string(),
	views: z.array(z.string()),
	code: z.string(),
	db_schema: z.string(),
})
export type Ruleset = z.infer<typeof Ruleset>

export function asyncRuleset(rulesetFn: () => string) {
	return Async.computedFetch(z.array(Ruleset), rulesetFn)
}

export function asyncView(pathFn: () => string | undefined) {
	return Async.computedFetch(z.string(), () => {
		const path = pathFn()
		if (!path) return undefined
		return SERVER_PREFIX + `/fn/view/${path}`
	})
}
