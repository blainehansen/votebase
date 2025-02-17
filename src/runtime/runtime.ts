// import z from 'zod'

export type CandidateSelfReplacement = {
	code: string,
	db_schema: string,
	db_migration: string,
}

const { core } = Deno as unknown as { core: {
	print: (message: string, is_error: boolean) => void,
	ops: {
		op_fetch: (url: string) => Promise<string>,
		op_register_fn: (name: string, isAction: boolean, func: (arg: unknown) => string | undefined) => void,
		op_set_timeout: (delay: number | undefined) => Promise<void>,
		op_propose_self_replacement: (candidate: CandidateSelfReplacement) => Promise<string>,
	},
} }

declare global {
	namespace votebase {
		function fetch(url: string): Promise<string>;
		function registerAction(name: string, func: (arg: unknown) => string | undefined): void;
		function registerView<Query>(name: string, func: (query: unknown) => string): void;
		function proposeSelfReplacement(candidate: CandidateSelfReplacement): Promise<string>;
	}
}

globalThis.votebase = {
	fetch(url) {
		return core.ops.op_fetch(url)
	},
	registerAction(name, func: (arg: unknown) => string | undefined) {
		core.ops.op_register_fn(name, true, func)
	},
	registerView(name, func: (query: unknown) => string) {
		core.ops.op_register_fn(name, false, func)
	},
	proposeSelfReplacement(candidate) {
		return core.ops.op_propose_self_replacement(candidate)
	},
}

globalThis.setTimeout = (callback, delay) => {
	core.ops.op_set_timeout(delay).then(callback)
	return 0
}


function argsToMessage(...args: unknown[]) {
	return args.map(arg => JSON.stringify(arg)).join(" ")
}

globalThis.console = {
	...globalThis.console,
	log: (...args: unknown[]) => {
		core.print(`[out]: ${argsToMessage(...args)}\n`, false)
	},
	error: (...args: unknown[]) => {
		core.print(`[err]: ${argsToMessage(...args)}\n`, true)
	},
}
