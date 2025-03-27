// import z from 'zod'
// import { zodToJsonSchema } from 'zod-to-json-schema'

export type Result<T, E = string> =
	| { ok: true, value: T }
	| { ok: false, error: E }

export type CandidateSelfReplacement = {
	code: string,
	db_schema: string,
	db_migration: string,
}

export type JsonPrimitive = string | number | boolean | null
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue }

const { core } = (globalThis as any).Deno as { core: {
	print: (message: string, is_error: boolean) => void,
	ops: {
		op_fetch: (url: string) => Promise<string>,
		// TODO add userId: string arg to all the actions/views
		op_register_fn: <T>(name: string, isAction: boolean, func: (arg: T) => Promise<string | void>) => void,
		// TODO need to figure out what the necessary rust interface is
		op_register_recurring_action: () => void,
		op_schedule_recurring_action: () => Promise<string>,
		op_schedule_action: (description: string, scheduled_time: string, action_name: string, action_arg: JsonValue) => Promise<string>,
		op_unschedule_action: (uuid: string) => Promise<void>,

		op_enroll_member: (email: string) => Promise<string>,
		op_remove_member_by_email: (email: string) => Promise<void>,
		op_remove_member_by_uuid: (uuid: string) => Promise<void>,

		// op_set_timeout: (delay: number | undefined) => Promise<void>,
		op_propose_self_replacement: (candidate: CandidateSelfReplacement) => Promise<string>,
	},
} }

// make these have truly private members? or add some special symbol?
export type FnAction<A extends JsonValue> = Readonly<{ name: string, isAction: true, func: (arg: A) => Promise<string | void> }>
export type FnView<Q extends JsonValue> = Readonly<{ name: string, isAction: false, func: (query: Q) => Promise<string> }>

export type Fn<T extends JsonValue> = FnAction<T> | FnView<T>

export type Hour = 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23

export type RecurringAction = {
	description: string,
	start: Date,
	hour: Hour,
	frequencyGranularity: 'day' | 'week' | 'month' | 'year',
	frequencyMultiplier: number,
	callAction: FnAction<null>,
}

declare global {
	namespace votebase {
		// schema: z.ZodSchema<Arg>,
		function Action<Arg extends JsonValue>(name: string, func: (arg: Arg) => Promise<string | void>): FnAction<Arg>
		// schema: z.ZodSchema<Query>,
		function View<Query extends JsonValue>(name: string, func: (query: Query) => Promise<string>): FnView<Query>

		// function RecurringAction(definition: RecurringAction): void

		// function scheduleRecurringAction(definition: RecurringAction): Promise<{ uuid: string }>
		function scheduleAction<Arg extends JsonValue>(description: string, at: Date, action: FnAction<Arg>, arg: Arg): Promise<string>
		function unscheduleAction(uuid: string): Promise<void>

		// TODO right now there's only *capability* for a single ruleset, so would it make sense for this to just add it to root no matter what?
		function enrollMember(email: string): Promise<string>
		function removeMemberByEmail(email: string): Promise<void>
		function removeMemberByUuid(uuid: string): Promise<void>

		// function addMemberToRuleset(memberUuid: string, rulesetFullPath: string): Promise<Result<void>>
		// function removeMemberFromRuleset(memberUuid: string, rulesetFullPath: string): Promise<Result<void>>

		// TODO needs to account for possibility of failure
		function proposeSelfReplacement(candidate: CandidateSelfReplacement): Promise<string>

		// function proposeChildRuleset(): Promise<void>
		// function instituteChildRuleset(): Promise<void>
	}
}
globalThis.votebase = {
	// registerAction(name, schema, func) {
	Action<Arg>(name: string, func: (arg: Arg) => Promise<string | void>) {
		// const jsonSchema = zodToJsonSchema(schema)
		const isAction = true
		// core.ops.op_register_fn(name, jsonSchema, isAction, func)
		core.ops.op_register_fn(name, isAction, func)
		return { name, isAction, func }
	},
	// registerView(name, schema, func) {
	View<Query>(name: string, func: (query: Query) => Promise<string>) {
		// const jsonSchema = zodToJsonSchema(schema)
		const isAction = false
		// core.ops.op_register_fn(name, jsonSchema, isAction, func)
		core.ops.op_register_fn(name, isAction, func)
		return { name, isAction, func }
	},
	// RecurringAction({ description, start, hour, frequencyGranularity, frequencyMultiplier, callAction }) {
	// 	core.ops.op_register_recurring_action(
	// 		description, start, hour, frequencyGranularity, frequencyMultiplier, callAction,
	// 	)
	// },

	// async scheduleRecurringAction({ description, start, hour, frequencyGranularity, frequencyMultiplier, callAction }) {
	// 	const uuid = await core.ops.op_schedule_recurring_action(
	// 		description, start, hour, frequencyGranularity, frequencyMultiplier, callAction,
	// 	)
	// 	return { uuid }
	// },
	scheduleAction<A extends JsonValue>(description: string, at: Date, action: FnAction<A>, arg: A) {
		// TODO check that the date is in the future
		return core.ops.op_schedule_action(description, at.toISOString(), action.name, arg)
	},
	unscheduleAction(uuid: string) {
		return core.ops.op_unschedule_action(uuid)
	},

	enrollMember(email: string) {
		return core.ops.op_enroll_member(email)
	},
	removeMemberByEmail(email) {
		return core.ops.op_remove_member_by_email(email)
	},
	removeMemberByUuid(uuid) {
		return core.ops.op_remove_member_by_uuid(uuid)
	},

	proposeSelfReplacement(candidate) {
		return core.ops.op_propose_self_replacement(candidate)
	},
}

declare global {
	function fetch(url: string): Promise<string>
}
globalThis.fetch = function fetch(url) {
	return core.ops.op_fetch(url)
}


// declare global {
// 	function setTimeout(callback: () => unknown): Promise<string>
// }
// globalThis.setTimeout = (callback, delay) => {
// 	core.ops.op_set_timeout(delay).then(callback)
// 	return 0
// }


declare global {
	namespace console {
		function log(...args: unknown[]): void
		function error(...args: unknown[]): void
	}
}
globalThis.console = {
	log: (...args) => {
		core.print(`[out]: ${argsToMessage(...args)}\n`, false)
	},
	error: (...args) => {
		core.print(`[err]: ${argsToMessage(...args)}\n`, true)
	},
}

function argsToMessage(...args: unknown[]) {
	return args.map(arg => JSON.stringify(arg)).join(" ")
}
