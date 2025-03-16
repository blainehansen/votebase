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
export function asyncRulesetListings() {
	return Async.fetch(z.array(RulesetListing), SERVER_PREFIX + '/rulesets')
}

export const RulesetDetail = z.strictObject({
	views: z.array(z.string()),
	code: z.string(),
	db_schema: z.string(),
})
export type RulesetDetail = z.infer<typeof RulesetDetail>
export function asyncRulesetDetail(rulesetFullPathFn: () => string) {
	return Async.computedFetch(RulesetDetail, () => SERVER_PREFIX + `/ruleset-detail/${rulesetFullPathFn()}`)
}

export function asyncRulesetViews(rulesetFullPathFn: () => string) {
	return Async.computedFetch(z.array(z.string()), () => SERVER_PREFIX + `/ruleset-views/${rulesetFullPathFn()}`)
}

export function asyncViewTemplate(pathFn: () => string) {
	return Async.computedFetch(z.string(), () => SERVER_PREFIX + `/fn/view/${pathFn()}`)
}
