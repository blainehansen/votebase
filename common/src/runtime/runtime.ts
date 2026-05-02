// import z from 'zod'
// import { zodToJsonSchema } from 'zod-to-json-schema'

type PromiseOr<T> = T | Promise<T>

const { core } = (globalThis as any).Deno as { core: {
	print: (message: string, is_error: boolean) => void,
	ops: {
		op_register_action: <T>(name: string, func: (arg: T, userId: string | null) => PromiseOr<ReplaceSelfStruct | void>) => void;
		op_register_view: <T>(name: string, func: (arg: T, userId: string | null) => PromiseOr<string>) => void;

		// TODO cutting dynamic scope for now
		// op_create_recurring_action: (description: string, start: string, recurrenceGranularity: RecurrenceGranularity, recurrenceMultiplier: number, action_name: string, action_arg: JsonValue) => Promise<string>,
		// op_remove_recurring_action: (uuid: string) => Promise<void>,

		// op_schedule_action: (description: string, scheduled_time: string, action_name: string, action_arg: JsonValue) => Promise<string>,
		// op_unschedule_action: (uuid: string) => Promise<void>,

		op_enroll_member: (email: string) => Promise<string>,
		op_remove_member_by_email: (email: string) => Promise<void>,
		op_remove_member_by_uuid: (uuid: string) => Promise<void>,

		op_propose_self_replacement: (candidate: BundledRuleset) => Promise<string>,
		// replacing self is always done by returning the candidate uuid from an action

		// // these two create and destroy rulesets entirely. they cannot create or destroy static children
		// // this initial ruleset is expected to have db_schema == db_migration, because this ruleset didn't previously exist, there's nothing to migrate
		// op_create_child_ruleset: (name: string, initial: BundledRuleset) => Promise<string>,
		// // all of the children, static and dynamic, are deleted here as well
		// op_delete_child_ruleset: (name: string) => Promise<void>,

		// // this is just the child version of op_propose_self_replacement
		// op_propose_child_replacement: (name: string, candidate: BundledRuleset) => Promise<string>,
		// // the table with candidate_id already has the name and full_path etc to know where it's headed
		// op_replace_child: (candidate_id: string) => Promise<void>,

		op_sql_fetch_all: <P extends ParamHint[], R extends FullRetHint>
			(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R>[]>,
		op_sql_fetch_one: <P extends ParamHint[], R extends FullRetHint>
			(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R>>,
		op_sql_fetch_optional: <P extends ParamHint[], R extends FullRetHint>
			(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R> | null>,
		op_sql_execute_statement: <P extends ParamHint[]>
			(sql: string, params: ActualParams<P>, hints: P) => Promise<number>,
		op_sql_execute_statements: (sql: string) => Promise<void>,

		// op_set_timeout: (delay: number | undefined) => Promise<void>,
		// TODO pull all the types from the standard, or even better have perplexity do it?
		// https://github.com/microsoft/TypeScript/blob/main/src/lib/dom.generated.d.ts
		// fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>;
		op_fetch: (url: string) => Promise<string>,
	},
} }


// export type Result<T, E = string> =
// 	| { ok: true, value: T }
// 	| { ok: false, error: E }

// type Dict<T> = { [key: string]: T }

type BundledRuleset = Readonly<{
	/**
	 * Typescript code containing all the Actions and Views of the `Ruleset`.
	*/
	ts_code: string,

	// this is truly harvested from the code, but honestly it might be a good idea to also require a declaration we can check against
	// fns: { [fn_name: string]: VotebaseFn<JsonValue> },
	// is there a world where the fns are all declared separately, and then some "shared code" chunk also? how to do this? create temp files for each to do all the checking?

	/**
	 * The final intended database schema.
	 * Used to check that `db_migration` does what it's intended to.
	*/
	db_schema: string,
	/**
	 * The migration intended to actually be run to reach the state of `db_schema`.
	 * This will be checked to ensure it actually goes from the *current* state of the `Ruleset` database to the one declared in `db_schema`.
	*/
	db_migration: string,
	// /**
	//  * The fully qualified names of all the database objects this `Ruleset` uses as its `requires`.
	// */
	// db_uses: string[],
	// /**
	//  * A mapping of the static children of this `Ruleset`, with some being simply `"keep"`, meaning to leave it as is.
	//  * If this `Ruleset` replaces the existing one, this will be the absolute state of the static children, with any existing ones changed to match their new description and extra ones recursively deleted.
	// */
	// static_children: Dict<KeepOrReplace<BundledRuleset>>,
	// /**
	//  * A mapping of the static recurring events of this `Ruleset`, with some being simply `"keep"`, meaning to leave it as is.
	//  * If this `Ruleset` replaces the existing one, this will be the absolute state of the static recurring events, with any existing ones changed to match their new description and extra ones deleted.
	// */
	// static_recurring_events: Dict<KeepOrReplace<StaticRecurringEvent>>,

	// TODO dynamic children and and events is scope I'm cutting for now
	// /**
	//  * A predicate that determines what dynamic children to keep.
	//  * All others will be recursively deleted.
	// */
	// dynamic_children_keep_rule: string,
	// /**
	//  * A predicate that determines what dynamic recurring events to keep.
	//  * All others will be deleted.
	// */
	// dynamic_recurring_event_keep_rule: string,
	// /**
	//  * A predicate that determines what scheduled events to keep.
	//  * All others will be deleted.
	// */
	// dynamic_standalone_event_keep_rule: string,
}>

export type StaticRecurringEvent = Readonly<{
	description: string, start: string, recurrenceGranularity: RecurrenceGranularity, recurrenceMultiplier: number,
	action_name: string, action_arg: JsonValue,
}>

export type KeepOrReplace<T> = 'keep' | T


/**
 * This is the type you should return from an `Action` when you want to replace the current Ruleset with the one pointed to by `replace_self_with_uuid`.
*/
export type ReplaceSelfStruct = { replace_self_with_uuid: string, delete_other_candidates: boolean }

// make these have truly private members? or add some special symbol?
export type FnAction<A extends JsonValue> = Readonly<{
	name: string, isAction: true,
	func: (arg: A, userId: string | null) => PromiseOr<ReplaceSelfStruct | void>,
}>
export type FnView<Q extends JsonValue> = Readonly<{
	name: string, isAction: false,
	func: (query: Q, userId: string | null) => PromiseOr<string>,
}>

export type VotebaseFn<T extends JsonValue> = FnAction<T> | FnView<T>

export type RecurrenceGranularity = 'Day' | 'Week' | 'Month' | 'Year'

export type RecurringAction<Arg extends JsonValue> = {
	description: string,
	start: Date,
	recurrenceGranularity: RecurrenceGranularity,
	recurrenceMultiplier: number,
	action: FnAction<Arg>,
	arg: Arg,
}

declare global {
	namespace votebase {
		/**
		 * Register an `Action` for your `Ruleset`.
		 * Actions are allowed to actually modify data or perform "mutating" operations on the database.
		 * If you return a `ReplaceSelfStruct`, the current `Ruleset` will be replaced by the one pointed to by `replace_self_with_uuid`, which must have been previously proposed with `proposeSelfReplacement`.
		*/
		// schema: z.ZodSchema<Arg>,
		function Action<Arg extends JsonValue>(name: string, func: (arg: Arg, userId: string | null) => PromiseOr<ReplaceSelfStruct | void>): FnAction<Arg>
		/**
		 * Register a View for your Ruleset.
		 * View are only allowed to read data in the database.
		*/
		// schema: z.ZodSchema<Query>,
		function View<Query extends JsonValue>(name: string, func: (query: Query, userId: string | null) => PromiseOr<string>): FnView<Query>

		// function RecurringAction(definition: RecurringAction): void

		// /**
		//  * Create a recurring schedule for an Action to be called repeatedly.
		//  * Allows you to update the state of the Ruleset on a regular schedule.
		// */
		// function createRecurringAction<Arg extends JsonValue>(definition: RecurringAction<Arg>): Promise<string>
		// /**
		//  * Delete a previously created recurring Action.
		// */
		// function removeRecurringAction(uuid: string): Promise<void>
		// /**
		//  * Schedule an Action to occur at some single point in the future.
		// */
		// function scheduleAction<Arg extends JsonValue>(description: string, at: Date, action: FnAction<Arg>, arg: Arg): Promise<string>
		// /**
		//  * Cancel an Action that was previously scheduled to occur at some single point in the future.
		// */
		// function unscheduleAction(uuid: string): Promise<void>

		/**
		 * Enroll a new member in the server.
		 * Returns the new uuid of the member.
		*/
		function enrollMember(email: string): Promise<string>
		/**
		 * Remove a member from the server, using their email to identify them.
		*/
		function removeMemberByEmail(email: string): Promise<void>
		/**
		 * Remove a member from the server, using their uuid to identify them.
		*/
		function removeMemberByUuid(uuid: string): Promise<void>

		// TODO membership in Rulesets is determined by the *declarative* membership predicate system, so when that gets designed I'll figure out a set of runtime functions for adding *properties* to members, based on the declared existing properties

		/**
		 * Propose a concrete `Ruleset` to replace the one that is currently running.
		 * To actually *use* this proposal and replace the current `Ruleset`, return a `ReplaceSelfStruct` in an `Action`.
		*/
		// TODO needs to account for possibility of failure
		function proposeSelfReplacement(candidate: BundledRuleset): Promise<string>

		// /**
		//  * Create a dynamic child `Ruleset` underneath the current one.
		//  * Returns the ruleset name, which can be used as an id.
		// */
		// function createChildRuleset(name: string, initial: BundledRuleset): Promise<string>
		// /**
		//  * Delete a dynamic child `Ruleset` underneath the current one, using the name returned by `createChildRuleset`.
		//  * All child `Ruleset`s of this one will also be recursively deleted.
		// */
		// function deleteChildRuleset(name: string): Promise<void>
		// /**
		//  * Propose a replacement for a dynamic child `Ruleset`.
		//  * Returns the proposal id that can be given to `replaceChild` to actually replace the child.
		// */
		// function proposeChildReplacement(name: string, candidate: BundledRuleset): Promise<string>
		// /**
		//  * Replaces a dynamic child `Ruleset` with the given candidate id previously received from `proposeChildReplacement`.
		//  * You don't need to provide a `name`, since that was already given to `proposeChildReplacement`.
		// */
		// function replaceChild(uuid: string): Promise<void>
	}
}
globalThis.votebase = {
	Action<Arg>(name: string, func: (arg: Arg, userId: string | null) => PromiseOr<ReplaceSelfStruct | void>) {
		// const jsonSchema = zodToJsonSchema(schema)
		core.ops.op_register_action(name, func)
		return { name, isAction: true, func }
	},
	View<Query>(name: string, func: (query: Query, userId: string | null) => PromiseOr<string>) {
		// const jsonSchema = zodToJsonSchema(schema)
		core.ops.op_register_view(name, func)
		return { name, isAction: false, func }
	},

	// createRecurringAction({ description, start, recurrenceGranularity, recurrenceMultiplier, action, arg }) {
	// 	return core.ops.op_create_recurring_action(
	// 		description, start.toISOString(), recurrenceGranularity, recurrenceMultiplier, action.name, arg,
	// 	)
	// },
	// removeRecurringAction(uuid) {
	// 	return core.ops.op_remove_recurring_action(uuid)
	// },
	// scheduleAction(description, at, action, arg) {
	// 	// TODO check that the date is in the future
	// 	return core.ops.op_schedule_action(description, at.toISOString(), action.name, arg)
	// },
	// unscheduleAction(uuid) {
	// 	return core.ops.op_unschedule_action(uuid)
	// },

	enrollMember(email) {
		return core.ops.op_enroll_member(email)
	},
	removeMemberByEmail(email) {
		return core.ops.op_remove_member_by_email(email)
	},
	removeMemberByUuid(uuid) {
		return core.ops.op_remove_member_by_uuid(uuid)
	},
	// addMembersToRuleset(full_path, uuids) {
	// 	return core.ops.op_add_members_to_ruleset(full_path, uuids)
	// },
	// // addMembersToRulesetByCondition(full_path, condition) {
	// // 	return core.ops.op_add_members_to_ruleset_by_condition(full_path, condition)
	// // },
	// removeMembersFromRuleset(full_path, uuids) {
	// 	return core.ops.op_remove_members_from_ruleset(full_path, uuids)
	// },

	proposeSelfReplacement(candidate) {
		return core.ops.op_propose_self_replacement(candidate)
	},

	// createChildRuleset(name, initial) {
	// 	return core.ops.op_create_child_ruleset(name, initial)
	// },
	// deleteChildRuleset(name) {
	// 	return core.ops.op_delete_child_ruleset(name)
	// },
	// proposeChildReplacement(name, candidate) {
	// 	return core.ops.op_propose_child_replacement(name, candidate)
	// },
	// replaceChild(candidate_id) {
	// 	return core.ops.op_replace_child(candidate_id)
	// },
}


export type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue }

declare global {
	namespace __ {
		export class QueryExecutor<P extends ParamHint[], R extends FullRetHint> {
			constructor(
				sql: string,
				hints: P,
				ret: R,
			)

			fetchAll(...params: ActualParams<P>): Promise<ActualRet<R>[]>
			fetchOne(...params: ActualParams<P>): Promise<ActualRet<R>>
			fetchOptional(...params: ActualParams<P>): Promise<ActualRet<R> | null>
		}
		export function StatementExecutor<P extends ParamHint[]>(sql: string, hints: P): (...params: ActualParams<P>) => Promise<number>
		export function StatementsExecutor(sql: string): () => Promise<void>
	}
}
globalThis.__ = {
	QueryExecutor: class QueryExecutor<P extends ParamHint[], R extends FullRetHint> {
		constructor(
			private readonly sql: string,
			private readonly hints: P,
			private readonly ret: R,
		) {}

		fetchAll(...params: ActualParams<P>): Promise<ActualRet<R>[]> {
			return core.ops.op_sql_fetch_all<P, R>(this.sql, params, this.hints, this.ret)
		}
		fetchOne(...params: ActualParams<P>): Promise<ActualRet<R>> {
			return core.ops.op_sql_fetch_one<P, R>(this.sql, params, this.hints, this.ret)
		}
		fetchOptional(...params: ActualParams<P>): Promise<ActualRet<R> | null> {
			return core.ops.op_sql_fetch_optional<P, R>(this.sql, params, this.hints, this.ret)
		}
	},
	StatementExecutor: function StatementExecutor<P extends ParamHint[]>(sql: string, hints: P): (...params: ActualParams<P>) => Promise<number> {
		return (...params) => {
			return core.ops.op_sql_execute_statement(sql, params, hints)
		}
	},
	StatementsExecutor: function StatementsExecutor(sql: string): () => Promise<void> {
		return () => {
			return core.ops.op_sql_execute_statements(sql)
		}
	},
}


export type RetPrimitiveTypeHintMap = {
	'Json': JsonValue,
	'Bool': boolean,
	'Text': string,
	// 'Bytea': Uint8Array,
	'Bytea': number[],
	'Hstore': { [key: string]: string | null },
	'I16': number,
	'I32': number,
	'I64': bigint,
	'F32': number,
	'F64': number,
	'Numeric': number,
	'Money': number,
}
export type RetPrimitiveTypeHint = keyof RetPrimitiveTypeHintMap
export type RetNullableTypeHint = `${RetPrimitiveTypeHint}?`
export type RetArrayTypeHint = `${RetPrimitiveTypeHint}[]`

export type RetHint = RetPrimitiveTypeHint | RetNullableTypeHint | RetArrayTypeHint

export type ParamPrimitiveTypeHintMap = {
	'Json': JsonValue,
	'Bool': boolean,
	'Text': string,
	// 'Bytea': number[] | Uint8Array,
	'Bytea': number[],
	'Hstore': { [key: string]: string | null },
	'I16': number,
	'I32': number,
	'I64': number | bigint,
	'F32': number,
	'F64': number,
	'Numeric': number,
	'Money': number,
}
export type ParamPrimitiveTypeHint = keyof ParamPrimitiveTypeHintMap
export type ParamNullableTypeHint = `${ParamPrimitiveTypeHint}?`
export type ParamArrayTypeHint = `${ParamPrimitiveTypeHint}[]`

export type ParamHint = ParamPrimitiveTypeHint | ParamNullableTypeHint | ParamArrayTypeHint


export type TypeOfParamHint<H extends ParamHint> =
	H extends `${infer P}?` ? (P extends ParamPrimitiveTypeHint ? ParamPrimitiveTypeHintMap[P] | null : never)
	: H extends `${infer P}[]` ? (P extends ParamPrimitiveTypeHint ? ParamPrimitiveTypeHintMap[P][] : never)
	:
	H extends ParamPrimitiveTypeHint ? ParamPrimitiveTypeHintMap[H]
	: never

type ActualParams<Hints extends ParamHint[]> = { [I in keyof Hints]: TypeOfParamHint<Hints[I]> }


export type TypeOfRetHint<H extends RetHint> =
	H extends `${infer P}?` ? (P extends RetPrimitiveTypeHint ? RetPrimitiveTypeHintMap[P] | null : never)
	: H extends `${infer P}[]` ? (P extends RetPrimitiveTypeHint ? RetPrimitiveTypeHintMap[P][] : never)
	:
	H extends RetPrimitiveTypeHint ? RetPrimitiveTypeHintMap[H]
	: never


type FullRetHint = RetHint | [string, RetHint][]

type ActualRet<R extends FullRetHint> =
	R extends RetHint ? TypeOfRetHint<R>
	: { [K in R[number][0]]: TypeOfRetHint<Extract<R[number], [K, unknown]>[1]> }




// // it feels like this concept of a generic ruleset is literally only valuable for the "ruleset library ecosystem"
// // when an actual ruleset shows up for a specific place and use, it's fully concrete, no more vars or vals
// type GenericRuleset = Omit<BundledRuleset, 'static_children'> & {
// 	// this is a bunch of "typed vars" that we have to fill in in both db_schema and db_migration
// 	// since these are types, we can dummy together expressions that minimally satisfy them when we're checking the ruleset
// 	// these are intended to be used as "relationships" or links to other rulesets. tables and functions and columns etc can be used from other rulesets
// 	// the main thing I'm trying to enable is the persistent weights in the kernel, with child rulesets using them to actually make specific decisions
// 	db_schema_vars: { [var_name: string]: PgType },

// 	// this tells us to either apply the migration implied by the GenericRuleset, or leave it alone, or if not present in this map delete it
// 	// maybe it makes more sense to require fully specifying 'delete' rather than allowing absence, it's more explicit
// 	static_children: { [child_name: string]: GenericRuleset | 'keep' },
// }

// type CompilableRuleset = Omit<GenericRuleset, 'static_children'> & {
// 	db_schema_vals: { [val_name: string]: PgExpr },

// 	static_children: { [child_name: string]: CompilableRuleset | 'keep' },
// }

// type PgType = 'Text' | 'Bool' | 'etc TODO'
// type PgExpr = string


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
	// this is actually the correct interface... but should we stringify it better? or even better, make the `print` command do the stringification at the v8 level?
	// https://github.com/microsoft/TypeScript/blob/main/src/lib/dom.generated.d.ts
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
