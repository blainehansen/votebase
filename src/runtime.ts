const { core } = Deno as unknown as {  core: {
	print: (message: string, is_error: boolean) => void,
	ops: {
		op_fetch: (url: string) => Promise<string>,
		op_register_func: (name: string, func: () => void) => void,
		op_set_timeout: (delay: number | undefined) => Promise<void>,
	},
} }

function argsToMessage(...args: unknown[]) {
	return args.map((arg) => JSON.stringify(arg)).join(" ")
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
		function reg(name: string, func: () => void): void;
	}
}

globalThis.votebase = {
	fetch(url) {
		return core.ops.op_fetch(url)
	},
	reg(name, func) {
		core.ops.op_register_func(name, func)
	},
}

globalThis.setTimeout = (callback, delay) => {
	core.ops.op_set_timeout(delay).then(callback)
	return 0
}
