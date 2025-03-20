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

export type CandidateRuleset = {
	code: string,
	db_schema: string,
	db_migration: string,
}

const { core } = globalThis.Deno as unknown as { core: {
	print: (message: string, is_error: boolean) => void,
	ops: {
		op_fetch: (url: string) => Promise<string>,
		op_register_fn: (name: string, isAction: boolean, func: (arg: unknown) => Promise<string | void>) => void,
		op_set_timeout: (delay: number | undefined) => Promise<void>,
		op_propose_self_replacement: (candidate: CandidateSelfReplacement) => Promise<string>,
	},
} }

export type FnAction<A> = { isAction: true, func: (arg: A) => Promise<string | void> }
export type FnView<Q> = { isAction: false, func: (query: Q) => Promise<string> }

export type Fn<T> =
	| FnAction<T>
	| FnView<T>

type BlankFns = { [key: string]: Fn<unknown> }

type NullActionsOf<Fns extends BlankFns> =
	{ [K in keyof Fns as Fns[K] extends FnAction<null> ? K : never]: K }

export type RecurringEvent<Fns extends BlankFns> = {
	start: Date,
	// hour?: 1 | 2,
	frequencyGranularity: 'day' | 'week' | 'month' | 'year',
	frequencyMultiplier: number,
	callAction: keyof NullActionsOf<Fns>,
}

// votebase.Action()

	// {
	// 		return { isAction: true, func }
	// 	}

declare global {

	// function fetch(): void

	// function setTimeout(callback: () => unknown): Promise<string>

	namespace votebase {
		// TODO fetch should go on the global namespace
		function fetch(url: string): Promise<string>


		function Action<Arg>(name: string, func: (arg: Arg) => Promise<string | void>): Fn<Arg>
		function View<Query>(name: string, func: (query: Query) => Promise<string>): Fn<Query>

		// TODO this would be a dramatic departure from how you're doing things now
		// it's much harder to validate from the rust side?
		// maybe have Action be the registrar *and* it returns an object that can be reused
		function defineRuleset<Fns extends BlankFns>(fns: Fns, recurringEvents: RecurringEvent<Fns>): void

		function registerAction(name: string, func: (arg: unknown) => Promise<string | void>): void
		function registerView(name: string, func: (query: unknown) => Promise<string>): void
		// function registerAction<Arg>(name: string, schema: z.ZodSchema<Arg>, func: (arg: Arg) => Promise<string | void>): void
		// function registerView<Query>(name: string, schema: z.ZodSchema<Query>, func: (query: Query) => Promise<string>): void

		// this is only allowed at the top level, intended to be similar to registerAction/View?
		// TODO this should require a well-typed FnAction<null>, which can only be created by the registration function
		function registerRecurringEvent(): void

		function scheduleEvent(at: Date, ): void
		function unscheduleEvent(): void

		// TODO right now there's only *capability* for a single ruleset, so would it make sense for this to just add it to root no matter what?
		function enrollMember(email: string): Promise<Result<{ uuid: string }>>
		function removeMember(memberUuid: string): Promise<Result<void>>

		// function addMemberToRuleset(memberUuid: string, rulesetFullPath: string): Promise<Result<void>>
		// function removeMemberFromRuleset(memberUuid: string, rulesetFullPath: string): Promise<Result<void>>

		// TODO needs to account for possibility of failure
		function proposeSelfReplacement(candidate: CandidateSelfReplacement): Promise<string>

		// function proposeChildRuleset(): Promise<void>
		// function instituteChildRuleset(): Promise<void>

		// function scheduleAction(at: Date, actionName: string, arg: unknown): Promise<string>
		// function cancelAction(uuid: string): Promise<void>
	}
}

// globalThis.votebase = {
// 	fetch(url) {
// 		return core.ops.op_fetch(url)
// 	},
// 	// registerAction(name, schema, func) {
// 	registerAction(name, func) {
// 		// const jsonSchema = zodToJsonSchema(schema)
// 		// core.ops.op_register_fn(name, jsonSchema, true, func)
// 		core.ops.op_register_fn(name, true, func)
// 	},
// 	// registerView(name, schema, func) {
// 	registerView(name, func) {
// 		// const jsonSchema = zodToJsonSchema(schema)
// 		// core.ops.op_register_fn(name, jsonSchema, false, func)
// 		core.ops.op_register_fn(name, false, func)
// 	},
// 	proposeSelfReplacement(candidate) {
// 		return core.ops.op_propose_self_replacement(candidate)
// 	},
// }

// globalThis.setTimeout = (callback, delay) => {
// 	core.ops.op_set_timeout(delay).then(callback)
// 	return 0
// }


// function argsToMessage(...args: unknown[]) {
// 	return args.map(arg => JSON.stringify(arg)).join(" ")
// }

// globalThis.console = {
// 	...globalThis.console,
// 	log: (...args: unknown[]) => {
// 		core.print(`[out]: ${argsToMessage(...args)}\n`, false)
// 	},
// 	error: (...args: unknown[]) => {
// 		core.print(`[err]: ${argsToMessage(...args)}\n`, true)
// 	},
// }
