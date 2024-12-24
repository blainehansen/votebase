import { differenceInMilliseconds } from 'date-fns'
type Dict<T> = { [key: string]: T }
type NonEmpty<T> = [T, ...T[]]

function isNonEmpty<T>(arr: T[]): arr is NonEmpty<T> {
	return arr.length !== 0
}

function demandEnv(variable: string) {
	const v = Deno.env.get(variable)
	if (!v) throw new Error(`env variable ${variable} must be set`)
	return v
}

const serverOptions = demandEnv("IS_PRODUCTION") === "true"
	? {
	  port: 443,
	  cert: Deno.readTextFileSync("./cert.pem"),
	  key: Deno.readTextFileSync("./key.pem"),
	}
	: { port: 8080, hostname: "0.0.0.0" }

function json(body: unknown, { status = 200, headers = {} }: ResponseInit = {}): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: {
			"content-type": "application/json; charset=utf-8",
			...headers,
		},
	})
}
function noContent() { return new Response(undefined, { status: 204 }) }

type ChangeFn = (newConstitution: Constitution<unknown>) => never

type Action<Data = unknown, Arg = unknown> = {
	argValidator: (raw: unknown) => Arg,
	allowedCheck: (data: Data, arg: Arg) => string[],
	fn: (change: ChangeFn, data: Data, arg: Arg) => Data,
}

type View<Data = unknown, Query = unknown> = {
	argValidator: (raw: unknown) => Query,
	// allowedCheck: (data: Data, query: Query) => string[],
	fn: (data: Data, query: Query) => Response,
}

// type Cons = new () => C

type Constitution<
	Data,
	Actions extends Dict<Action<Data>> = Dict<Action<Data>>,
	Views extends Dict<View<Data>> = Dict<View<Data>>,
	Subs extends Dict<LiveConstitution> = Dict<LiveConstitution>,
> = {
	readonly initializer: () => Data,
	readonly dataValidator: (data: Data) => string[],
	readonly actions: Actions,
	readonly views: Views,
	readonly subConstitutions: Subs,
}

class LiveConstitution<Data = unknown> {
	private data: Data
	constructor(
		readonly constitution: Constitution<Data>,
		private changeSelf: ChangeFn,
	) {
		this.data = constitution.initializer()
	}

	maybeAction(actionPath: NonEmpty<string>, rawArg: unknown) {
		const [first, ...rest] = actionPath

		if (isNonEmpty(rest)) {
			const subConstitution = Object.entries(this.constitution.subConstitutions).find(([name, ]) => name === first)
			if (!subConstitution)
				// TODO message
				throw new Error()
			subConstitution[1].maybeAction(rest, rawArg)
			return
		}

		const action = Object.entries(this.constitution.actions).find(([name, ]) => name === first)
		if (!action)
			// TODO message
			throw new Error()
		this.executeAction(action[1], rawArg)
	}

	private executeAction<Arg>(action: Action<Data, Arg>, rawArg: unknown) {
		const arg = action.argValidator(rawArg)
		try {
			// TODO deep copy data
			const data = this.data
			const newData = action.fn(changeSelf, data, arg)
			const errors = this.constitution.dataValidator(newData)
			if (errors.length > 0)
				throw new Error()
			this.data = newData
			return noContent()
		}
		catch (e) {
			if (e instanceof ChangeSelfException)
				do the thing
			return json({ error: "" }, { status: 400 })
		}
	}
}

class ChangeSelfException extends Error {}
function changeSelf(): never { throw new ChangeSelfException() }

function fetchRootConstitution(): LiveConstitution {
	throw 'unimplemented'
}

const root = { root: fetchRootConstitution() }

function swapItem<D extends Dict<unknown>, K extends keyof D>(dictionary: D, key: K, value: D[K]) {
	dictionary[key] = value
}


const queuedEvents = new Map<string, [Date, string]>()

function queueEvent(key: string, scheduled: Date, value: string) {
	const now = new Date()
	const millisUntil = differenceInMilliseconds(scheduled, now)
	setTimeout(() => {
		console.log(value)
		queuedEvents.delete(key)
	}, millisUntil)
	queuedEvents.set(key, [scheduled, value])
}


setTimeout(() => {
	for (const [key, [scheduled, value]] of queuedEvents.entries())
		// this will double set the item, but I guess that's okay
		queueEvent(key, scheduled, value)
})

Deno.serve(serverOptions, req => {
	const url = new URL(req.url)
	console.log(url.pathname)

	return json({ message: "yoyo" })
})
