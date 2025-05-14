// for now we're not going to do hyper locked down secure pre-registered queries
// the hard part is that the code can always just pass whatever it wants, especially if it understands the real contract, which it always can
// it's not worth enforcing and thereby decreasing performance that much


// the cli has two commands:
// dev-prepare, that prepares all the sql and outputs a gitignored *dev* version of it, for consistency a namespace with exported consts and is then `export default`ed. or just kidding, if we're going to use string symbol names then it needs to be an `export default {}` with the right keys
// bundle, which also prepares all the sql and smushes it into the ruleset.ts file in place of the existing import, and puts all of that into a gitignored ruleset.json file

// the cli when called always does the same thing, it fully prepares all the sql, and generates both the dev version and

// , and for dev has an `import queries from './queries'`. the cli has a command to "package" a ruleset as a json encoded thing ready to send to a server, and part of what it does is strip out the `import queries from './queries` and in it's place insert the `namespace queries {}` (detect and use the name used in the import)
// - if the queries file is already formatted as `namespace queries {}\nexport default queries` then we can just smush the
// - probably the thing that makes sense is just on every generation call do both, generate the *dev* version of the file

// export type CandidateSelfReplacement = {
// 	code: string,
// 	db_schema: string,
// 	db_migration: string,
// }


const { core } = (globalThis as any).Deno as { core: {
	print: (message: string, is_error: boolean) => void,
	ops: {
		// op_fetch: (url: string) => Promise<string>,
		// // TODO add userId: string arg to all the actions/views
		// op_register_fn: <T>(name: string, isAction: boolean, func: (arg: T) => Promise<string | void>) => void,
		// // TODO need to figure out what the necessary rust interface is
		// // op_register_recurring_action: (name: string, ) => void,
		// op_create_recurring_action: (description: string, start: string, recurrenceGranularity: RecurrenceGranularity, recurrenceMultiplier: number, action_name: string, action_arg: JsonValue) => Promise<string>,
		// op_remove_recurring_action: (uuid: string) => Promise<void>,

		// op_schedule_action: (description: string, scheduled_time: string, action_name: string, action_arg: JsonValue) => Promise<string>,
		// op_unschedule_action: (uuid: string) => Promise<void>,

		// op_enroll_member: (email: string) => Promise<string>,
		// op_remove_member_by_email: (email: string) => Promise<void>,
		// op_remove_member_by_uuid: (uuid: string) => Promise<void>,

		// // op_set_timeout: (delay: number | undefined) => Promise<void>,
		// op_propose_self_replacement: (candidate: CandidateSelfReplacement) => Promise<string>,

		op_sql_fetch_all: <P extends TypeHint[], R extends RetHint>(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R>[]>,
		op_sql_fetch_one: <P extends TypeHint[], R extends RetHint>(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R>>,
		op_sql_fetch_optional: <P extends TypeHint[], R extends RetHint>(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R> | null>,
		op_sql_execute_statement: <P extends TypeHint[]>(sql: string, params: ActualParams<P>, hints: P) => Promise<number>,
		op_sql_execute_statements: (sql: string) => Promise<void>,
	},
} }


export type JsonPrimitive = string | number | boolean | null
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue }

type Dict<T> = { [key: string]: T }

type ActualParams<Hints extends TypeHint[]> = { [I in keyof Hints]: TypeOfHint<Hints[I]> }

type RetHint = TypeHint | [string, TypeHint][]
type ActualRet<R extends RetHint> =
	R extends TypeHint ? TypeOfHint<R>
	: { [K in R[number][0]]: TypeOfHint<Extract<R[number], [K, unknown]>[1]> }

class QueryExecutor<P extends TypeHint[], R extends RetHint> {
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
}

function StatementExecutor<P extends TypeHint[]>(sql: string, hints: P): (...params: ActualParams<P>) => Promise<number> {
	return (...params) => {
		return core.ops.op_sql_execute_statement(sql, params, hints)
	}
}

function StatementsExecutor(sql: string): () => Promise<void> {
	return () => {
		return core.ops.op_sql_execute_statements(sql)
	}
}

export type PrimitiveTypeHintMap = {
	'json': JsonValue,
	'bool': boolean,
	'text': string,
	'number': number,
	'int': number,
	'bigint': bigint,
}

// type PrimitiveParamTypeHintMap = {
// 	'json': JsonValue,
// 	'bool': boolean,
// 	'text': string,
// 	'double': number,
// 	// 'timestamp': Date,
// }


export type PrimitiveTypeHint = keyof PrimitiveTypeHintMap
export type NullableTypeHint = `${PrimitiveTypeHint}?`
export type ArrayTypeHint = `${PrimitiveTypeHint}[]`

export type TypeHint = PrimitiveTypeHint | NullableTypeHint | ArrayTypeHint

export type TypeOfHint<H extends TypeHint> =
	H extends `${infer P}?` ? (P extends PrimitiveTypeHint ? PrimitiveTypeHintMap[P] | null : never)
	: H extends `${infer P}[]` ? (P extends PrimitiveTypeHint ? PrimitiveTypeHintMap[P][] : never)
	: H extends PrimitiveTypeHint ? PrimitiveTypeHintMap[H]
	: never
