const { core } = Deno

function argsToMessage(...args) {
	return args.map((arg) => JSON.stringify(arg)).join(" ")
}

globalThis.console = {
	log: (...args) => {
		core.print(`[out]: ${argsToMessage(...args)}\n`, false)
	},
	error: (...args) => {
		core.print(`[err]: ${argsToMessage(...args)}\n`, true)
	},
}

globalThis.votebase = {
	fetch: url => {
		return core.ops.op_fetch(url)
	},
	reg: (name, func) => {
		core.ops.op_register_func(name, func)
	},
}

globalThis.setTimeout = async (callback, delay) => {
	core.ops.op_set_timeout(delay).then(callback)
}
