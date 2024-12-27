import { type Dict, type NonEmpty, isNonEmpty } from './utils.ts'
import { differenceInMilliseconds } from 'date-fns'
import { demandEnv, errorMessage, json, noContent, pathnameToList } from './serverUtils.ts'

class Action<Data, Arg> {
	constructor(
		private readonly argValidator: (raw: unknown) => Arg,
		private readonly allowedCheck: (data: Data, arg: Arg) => string[],
		private readonly fn: (data: Data, arg: Arg) => { change: true, next: OpaqueConstitution } | { change: false, data: Data },
	) {}

	execute(data: Data, rawArg: unknown): ReturnType<typeof this.fn> | string[] {
		const arg = this.argValidator(rawArg)
		// TODO checks and reporting
		const allowedErrors = this.allowedCheck(data, arg)
		if (allowedErrors) return allowedErrors
		return this.fn(data, arg)
	}
}
type OpaqueAction = Action<unknown, unknown>

class View<Data, Query> {
	constructor(
		private readonly queryValidator: (raw: unknown) => Query,
		// private readonly allowedCheck: (data: Data, query: Query) => string[],
		private readonly fn: (data: Data, query: Query) => Response,
	) {}

	execute(data: Data, rawQuery: unknown): ReturnType<typeof this.fn> {
		const query = this.queryValidator(rawQuery)
		// TODO checks and reporting
		// const allowedErrors = this.allowedCheck(data, query)
		// if (allowedErrors) return allowedErrors
		return this.fn(data, query)
	}
}
type OpaqueView = View<unknown, unknown>

class Constitution<
	Data,
	Actions extends Dict<Action<Data, unknown>>,
	Views extends Dict<View<Data, unknown>>,
	SubConstitutions extends Dict<OpaqueConstitution>,
> {
	constructor(
		readonly initializer: () => Data,
		private readonly dataValidator: (data: Data) => string[],
		private readonly actions: Actions,
		private readonly views: Views,
		private readonly subConstitutions: SubConstitutions,
	) {}

	executeAction(data: Data, actionName: string, rawQuery: unknown) {
		const action = Object.entries(this.actions).find(([name, ]) => name === actionName)
		if (!action) throw ''
		return action[1].execute(data, rawQuery)
	}

	executeView(data: Data, viewName: string, rawQuery: unknown) {
		const view = Object.entries(this.views).find(([name, ]) => name === viewName)
		if (!view) throw ''
		return view[1].execute(data, rawQuery)
	}
}
// deno-lint-ignore ban-types
type OpaqueConstitution = Constitution<unknown, {}, {}, {}>

class LiveConstitution {
	private data: unknown
	private readonly subLives: Map<string, LiveConstitution>
	constructor(
		readonly constitution: OpaqueConstitution,
		private readonly changeSelf: (newConstitution: OpaqueConstitution) => void,
	) {
		this.data = constitution.initializer()
		this.subLives = new Map(
			Object.entries(constitution.subConstitutions)
				.map(([key, value]) => [key, new LiveConstitution(value, n => changeItem(this.subLives, key, n))])
		)
	}

	static maybeView(rootConstitution: LiveConstitution, viewPath: NonEmpty<string>, rawQuery: unknown): Response {
		const path = viewPath.slice()

		let currentSegment: string | undefined
		let currentLive = rootConstitution
		// deno-lint-ignore no-cond-assign
		while (currentSegment = path.shift()) {
			if (path.length === 0)
				// expect view to exist
				return currentLive.executeView(currentSegment, rawQuery)
					?? errorMessage(`no view found at ${viewPath}`)

			// keep iterating
			const nextLive = Object.entries(currentLive.constitution.subConstitutions).find(([name, ]) => name === currentSegment)
			if (!nextLive) return errorMessage(`no view found at ${viewPath}`)
			currentLive = nextLive[1]
		}

		// wish I could write a proof that this is true!
		throw new Error(`should be impossible to end maybeView after loop: ${viewPath}`)
	}

	private executeView(viewName: string, rawQuery: unknown): Response | undefined {
		const foundView = Object.entries(this.constitution.views).find(([name, ]) => name === viewName)
		if (!foundView) return undefined
		const view = foundView[1]
		const query = view.queryValidator(rawQuery)
		// TODO validation errors instead of assumption of success
		return view.fn(this.data, query)
	}

	static maybeAction(rootConstitution: LiveConstitution, actionPath: NonEmpty<string>, rawArg: unknown): Response {
		const path = actionPath.slice()

		let currentSegment: string | undefined
		let currentLive = rootConstitution
		// deno-lint-ignore no-cond-assign
		while (currentSegment = path.shift()) {
			if (path.length === 0) {
				// expect action to exist
				const maybeErrors = currentLive.executeAction(currentSegment, rawArg)
				return maybeErrors
					// TODO better messaging
					? errorMessage(maybeErrors.length > 0 ? maybeErrors.toString() : `no action found at ${actionPath}`)
					: noContent()
			}

			// keep iterating
			const nextLive = Object.entries(currentLive.constitution.subConstitutions).find(([name, ]) => name === currentSegment)
			if (!nextLive) return errorMessage(`no action found at ${actionPath}`)
			currentLive = nextLive[1]
		}

		// wish I could write a proof that this is true!
		throw new Error(`should be impossible to end maybeAction after loop: ${actionPath}`)
	}

	private executeAction(actionName: string, rawArg: unknown): string[] | undefined {
		const foundAction = Object.entries(this.constitution.actions).find(([name, ]) => name === actionName)
		if (!foundAction) return []
		const action = foundAction[1]
		const arg = action.argValidator(rawArg)
		// TODO validation errors instead of assumption of success
		const errors = action.allowedCheck(this.data, arg)
		if (errors.length > 0) return errors

		const result = action.fn(this.data, arg)
		if (result.change)
			// do the change
			this.changeSelf(result.next)
		else
			// update the data
			this.data = result.data
		return
	}
}

function handleRequest(req: Request): Response {
	const url = new URL(req.url)
	const path = pathnameToList(url.pathname)
	if (!isNonEmpty(path))
		return errorMessage("no action or view specified")

	// if the request is a get, we're looking for a view
	if (req.method === 'GET') {
		const query = url.searchParams.get('query')
		return LiveConstitution.maybeView(constitutionTree.root, path, query ? JSON.parse(decodeURIComponent(query)) : null)
	}

	// if it's a post, we're looking for an action
	if (req.method === 'POST') {
		const arg = url.searchParams.get('arg')
		return LiveConstitution.maybeAction(constitutionTree.root, path, arg ? JSON.parse(decodeURIComponent(arg)) : null)
	}

	return errorMessage("", { status: 405, headers: { Allow: "GET, POST" } })
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

function fetchRootConstitution() {
	return new LiveConstitution(
		{
			initializer() { return null },
			dataValidator(_data) { return [] },
			actions: {},
			views: {},
			subConstitutions: {},
		},
		newConstitution => { changeItem(constitutionTree, 'root', newConstitution) },
	) as LiveConstitution
}

const constitutionTree = { root: fetchRootConstitution() }

function changeItem<D extends Dict<unknown>, K extends keyof D>(dictionary: D, key: K, value: D[K]) {
	dictionary[key] = value
}


// type Cons = new () => C
const serverOptions = demandEnv("IS_PRODUCTION") === "true"
	? {
	  port: 443,
	  cert: Deno.readTextFileSync("./cert.pem"),
	  key: Deno.readTextFileSync("./key.pem"),
	}
	: { port: 8080, hostname: "0.0.0.0" }

setTimeout(() => {
	for (const [key, [scheduled, value]] of queuedEvents.entries())
		// this will double set the item, but I guess that's okay
		queueEvent(key, scheduled, value)
})

Deno.serve(serverOptions, handleRequest)
