import z from 'zod'

const { core } = Deno as unknown as { core: {
	print: (message: string, is_error: boolean) => void,
	ops: {
		op_fetch: (url: string) => Promise<string>,
		op_register_action: (name: string, func: (arg: unknown) => number | undefined) => void,
		op_register_view: (name: string, func: (query: unknown) => string) => void,
		op_set_timeout: (delay: number | undefined) => Promise<void>,
	},
} }

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

declare global {
	namespace votebase {
		function fetch(url: string): Promise<string>;
		function registerAction(name: string, func: (arg: unknown) => number | undefined): void;
		function registerView<Query>(name: string, func: (query: unknown) => string): void;
	}
}

globalThis.votebase = {
	fetch(url) {
		return core.ops.op_fetch(url)
	},
	registerAction(name, func) {
		core.ops.op_register_action(name, func)
	},
	registerView(name, func) {
		core.ops.op_register_view(name, func)
	},
}

globalThis.setTimeout = (callback, delay) => {
	core.ops.op_set_timeout(delay).then(callback)
	return 0
}
