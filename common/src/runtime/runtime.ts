// import z from 'zod'
// import { zodToJsonSchema } from 'zod-to-json-schema'

// export type Result<T, E = string> =
// 	| { ok: true, value: T }
// 	| { ok: false, error: E }


type ConcreteRuleset = {
	code: string,
	db_schema: string,
	db_migration: string,

	// this is truly harvested from the code, but honestly it might be a good idea to also require a declaration we can check against
	// fns: { [fn_name: string]: VotebaseFn<JsonValue> },

	// static children are *at least* necessary for situations where in one ruleset you create an election for something that must be a ruleset! and one where that election can't change the ruleset itself that specifies that election
	// you literally can't do the idea of a constitutional tree with a kernel root without child rulesets,
	static_children: { [child_name: string]: ConcreteRuleset },

	static_recurring_actions: {
		description: string, start: string, recurrenceGranularity: RecurrenceGranularity, recurrenceMultiplier: number,
		action_name: string, action_arg: JsonValue,
	}[],
}

type CandidateRuleset = Omit<ConcreteRuleset, 'static_children'> & {
	static_children: { [child_name: string]: CandidateRuleset | 'keep' },
}

const { core } = (globalThis as any).Deno as { core: {
	print: (message: string, is_error: boolean) => void,
	ops: {
		// op_set_timeout: (delay: number | undefined) => Promise<void>,
		op_fetch: (url: string) => Promise<string>,

		op_register_fn: <T>(name: string, isAction: boolean, func: (arg: T, userId: string | null) => Promise<string | void>) => void,

		// TODO need to figure out what the necessary rust interface is
		op_create_recurring_action: (description: string, start: string, recurrenceGranularity: RecurrenceGranularity, recurrenceMultiplier: number, action_name: string, action_arg: JsonValue) => Promise<string>,
		op_remove_recurring_action: (uuid: string) => Promise<void>,

		op_schedule_action: (description: string, scheduled_time: string, action_name: string, action_arg: JsonValue) => Promise<string>,
		op_unschedule_action: (uuid: string) => Promise<void>,

		op_enroll_member: (email: string) => Promise<string>,
		op_remove_member_by_email: (email: string) => Promise<void>,
		op_remove_member_by_uuid: (uuid: string) => Promise<void>,

		// op_add_members_to_ruleset: (full_path: string, uuids: string[]) => Promise<void>,
		// // op_add_members_to_ruleset_by_condition: (full_path: string, condition: string) => Promise<void>,
		// op_remove_members_from_ruleset: (full_path: string, uuids: string[]) => Promise<void>,

		op_propose_self_replacement: (candidate: CandidateRuleset) => Promise<string>,
		// replacing self is always done by returning the candidate uuid from an action

		// these two create and destroy rulesets entirely. they cannot create or destroy static children
		// this initial ruleset is expected to have db_schema == db_migration, because this ruleset didn't previously exist, there's nothing to migrate
		op_create_child_ruleset: (name: string, initial: ConcreteRuleset) => Promise<string>,
		// all of the children, static and dynamic, are deleted here as well
		op_delete_child_ruleset: (name: string) => Promise<void>,

		// this is just the child version of op_propose_self_replacement
		op_propose_child_replacement: (name: string, candidate: CandidateRuleset) => Promise<string>,
		// the table with candidate_id already has the name and full_path etc to know where it's headed
		op_replace_child: (candidate_id: string) => Promise<void>,

		op_sql_fetch_all: <P extends ParamHint[], R extends FullRetHint>
			(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R>[]>,
		op_sql_fetch_one: <P extends ParamHint[], R extends FullRetHint>
			(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R>>,
		op_sql_fetch_optional: <P extends ParamHint[], R extends FullRetHint>
			(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R> | null>,
		op_sql_execute_statement: <P extends ParamHint[]>
			(sql: string, params: ActualParams<P>, hints: P) => Promise<number>,
		op_sql_execute_statements: (sql: string) => Promise<void>,
	},
} }

// make these have truly private members? or add some special symbol?
export type FnAction<A extends JsonValue> = Readonly<{
	name: string, isAction: true,
	func: (arg: A, userId: string | null) => Promise<string | void>,
}>
export type FnView<Q extends JsonValue> = Readonly<{
	name: string, isAction: false,
	func: (query: Q, userId: string | null) => Promise<string>,
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
		// schema: z.ZodSchema<Arg>,
		function Action<Arg extends JsonValue>(name: string, func: (arg: Arg, userId: string | null) => Promise<string | void>): FnAction<Arg>
		// schema: z.ZodSchema<Query>,
		function View<Query extends JsonValue>(name: string, func: (query: Query, userId: string | null) => Promise<string>): FnView<Query>

		// function RecurringAction(definition: RecurringAction): void

		function createRecurringAction<Arg extends JsonValue>(definition: RecurringAction<Arg>): Promise<string>
		function removeRecurringAction(uuid: string): Promise<void>
		function scheduleAction<Arg extends JsonValue>(description: string, at: Date, action: FnAction<Arg>, arg: Arg): Promise<string>
		function unscheduleAction(uuid: string): Promise<void>

		function enrollMember(email: string): Promise<string>
		function removeMemberByEmail(email: string): Promise<void>
		function removeMemberByUuid(uuid: string): Promise<void>

		// function addMembersToRuleset(full_path: string, uuids: string[]): Promise<void>
		// // function addMembersToRulesetByCondition(full_path: string, condition: string): Promise<void>
		// function removeMembersFromRuleset(full_path: string, uuids: string[]): Promise<void>

		function proposeSelfReplacement(candidate: CandidateRuleset): Promise<string>

		// TODO needs to account for possibility of failure
		function proposeSelfReplacement(candidate: CandidateRuleset): Promise<string>

		function createChildRuleset(name: string, initial: ConcreteRuleset): Promise<string>
		function deleteChildRuleset(name: string): Promise<void>
		function proposeChildReplacement(name: string, candidate: CandidateRuleset): Promise<string>
		function replaceChild(uuid: string): Promise<void>
	}
}
globalThis.votebase = {
	Action<Arg>(name: string, func: (arg: Arg, userId: string | null) => Promise<string | void>) {
		const isAction = true
		// const jsonSchema = zodToJsonSchema(schema)
		// core.ops.op_register_fn(name, jsonSchema, isAction, func)
		core.ops.op_register_fn(name, isAction, func)
		return { name, isAction, func }
	},
	View<Query>(name: string, func: (query: Query, userId: string | null) => Promise<string>) {
		const isAction = false
		// const jsonSchema = zodToJsonSchema(schema)
		// core.ops.op_register_fn(name, jsonSchema, isAction, func)
		core.ops.op_register_fn(name, isAction, func)
		return { name, isAction, func }
	},
	// RecurringAction({ description, start, hour, frequencyGranularity, frequencyMultiplier, callAction }) {
	// 	core.ops.op_register_recurring_action(
	// 		description, start, hour, frequencyGranularity, frequencyMultiplier, callAction,
	// 	)
	// },

	createRecurringAction({ description, start, recurrenceGranularity, recurrenceMultiplier, action, arg }) {
		return core.ops.op_create_recurring_action(
			description, start.toISOString(), recurrenceGranularity, recurrenceMultiplier, action.name, arg,
		)
	},
	removeRecurringAction(uuid) {
		return core.ops.op_remove_recurring_action(uuid)
	},
	scheduleAction(description, at, action, arg) {
		// TODO check that the date is in the future
		return core.ops.op_schedule_action(description, at.toISOString(), action.name, arg)
	},
	unscheduleAction(uuid) {
		return core.ops.op_unschedule_action(uuid)
	},

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

	createChildRuleset(name, initial) {
		return core.ops.op_create_child_ruleset(name, initial)
	},
	deleteChildRuleset(name) {
		return core.ops.op_delete_child_ruleset(name)
	},
	proposeChildReplacement(name, candidate) {
		return core.ops.op_propose_child_replacement(name, candidate)
	},
	replaceChild(candidate_id) {
		return core.ops.op_replace_child(candidate_id)
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




export type JsonPrimitive = string | number | boolean | null
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue }

// type Dict<T> = { [key: string]: T }


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
// type GenericRuleset = Omit<ConcreteRuleset, 'static_children'> & {
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
