// for now we're not going to do hyper locked down secure pre-registered queries
// the hard part is that the code can always just pass whatever it wants, especially if it understands the real contract, which it always can
// it's not worth enforcing and thereby decreasing performance that much

// , and for dev has an `import queries from './queries'`. the cli has a command to "package" a ruleset as a json file ready to send to a server, and part of what it does is strip out the `import queries from './queries` and in it's place insert the `namespace queries {}` (detect and use the name used in the import)
// - if the queries file is already formatted as `namespace queries {}\nexport default queries` then we can just smush the
// - probably the thing that makes sense is just on every generation call do both, generate the *dev* version of the file

// import z from 'zod'
// import { zodToJsonSchema } from 'zod-to-json-schema'

// export type Result<T, E = string> =
// 	| { ok: true, value: T }
// 	| { ok: false, error: E }

export type CandidateSelfReplacement = {
	code: string,
	db_schema: string,
	db_migration: string,
}

const { core } = (globalThis as any).Deno as { core: {
	print: (message: string, is_error: boolean) => void,
	ops: {
		op_fetch: (url: string) => Promise<string>,
		// TODO add userId: string arg to all the actions/views
		op_register_fn: <T>(name: string, isAction: boolean, func: (arg: T) => Promise<string | void>) => void,
		// TODO need to figure out what the necessary rust interface is
		// op_register_recurring_action: (name: string, ) => void,
		op_create_recurring_action: (description: string, start: string, recurrenceGranularity: RecurrenceGranularity, recurrenceMultiplier: number, action_name: string, action_arg: JsonValue) => Promise<string>,
		op_remove_recurring_action: (uuid: string) => Promise<void>,

		op_schedule_action: (description: string, scheduled_time: string, action_name: string, action_arg: JsonValue) => Promise<string>,
		op_unschedule_action: (uuid: string) => Promise<void>,

		op_enroll_member: (email: string) => Promise<string>,
		op_remove_member_by_email: (email: string) => Promise<void>,
		op_remove_member_by_uuid: (uuid: string) => Promise<void>,

		// op_set_timeout: (delay: number | undefined) => Promise<void>,
		op_propose_self_replacement: (candidate: CandidateSelfReplacement) => Promise<string>,

		op_propose_child_ruleset: () => Promise<string>,

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
export type FnAction<A extends JsonValue> = Readonly<{ name: string, isAction: true, func: (arg: A) => Promise<string | void> }>
export type FnView<Q extends JsonValue> = Readonly<{ name: string, isAction: false, func: (query: Q) => Promise<string> }>

export type Fn<T extends JsonValue> = FnAction<T> | FnView<T>

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
		function Action<Arg extends JsonValue>(name: string, func: (arg: Arg) => Promise<string | void>): FnAction<Arg>
		// schema: z.ZodSchema<Query>,
		function View<Query extends JsonValue>(name: string, func: (query: Query) => Promise<string>): FnView<Query>

		// function RecurringAction(definition: RecurringAction): void

		function createRecurringAction<Arg extends JsonValue>(definition: RecurringAction<Arg>): Promise<string>
		function removeRecurringAction(uuid: string): Promise<void>
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




export type JsonPrimitive = string | number | boolean | null
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue }

type Dict<T> = { [key: string]: T }


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
// export type RetNullableTypeHint = `${RetPrimitiveTypeHint}?`
// export type RetArrayTypeHint = `${RetPrimitiveTypeHint}[]`

export type RetHint = RetPrimitiveTypeHint // | RetNullableTypeHint | RetArrayTypeHint

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
// export type ParamNullableTypeHint = `${ParamPrimitiveTypeHint}?`
// export type ParamArrayTypeHint = `${ParamPrimitiveTypeHint}[]`

export type ParamHint = ParamPrimitiveTypeHint // | ParamNullableTypeHint | ParamArrayTypeHint


export type TypeOfParamHint<H extends ParamHint> =
	// H extends `${infer P}?` ? (P extends PrimitiveTypeHint ? PrimitiveTypeHintMap[P] | null : never)
	// : H extends `${infer P}[]` ? (P extends PrimitiveTypeHint ? PrimitiveTypeHintMap[P][] : never)
	// :
	H extends ParamPrimitiveTypeHint ? ParamPrimitiveTypeHintMap[H]
	: never

type ActualParams<Hints extends ParamHint[]> = { [I in keyof Hints]: TypeOfParamHint<Hints[I]> }


export type TypeOfRetHint<H extends RetHint> =
	// H extends `${infer P}?` ? (P extends PrimitiveTypeHint ? PrimitiveTypeHintMap[P] | null : never)
	// : H extends `${infer P}[]` ? (P extends PrimitiveTypeHint ? PrimitiveTypeHintMap[P][] : never)
	// :
	H extends RetPrimitiveTypeHint ? RetPrimitiveTypeHintMap[H]
	: never


type FullRetHint = RetHint | [string, RetHint][]

type ActualRet<R extends FullRetHint> =
	R extends RetHint ? TypeOfRetHint<R>
	: { [K in R[number][0]]: TypeOfRetHint<Extract<R[number], [K, unknown]>[1]> }
