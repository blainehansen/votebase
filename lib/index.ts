import { type Dict, type NonEmpty } from './utils.ts'
import { errorMessage, noContent } from './serverUtils.ts'

export class Action<Data, Arg> {
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
export type OpaqueAction = Action<unknown, unknown>

export class View<Data, Query> {
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
export type OpaqueView = View<unknown, unknown>

export class Constitution<
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

	makeLive(changeSelf: (c: LiveConstitution) => void) {
		const nested = new Map()
		for (const [key, child] of Object.entries(this.subConstitutions))
			nested.set(key, child.makeLive(c => { nested.set(key, c) }))

		const data = this.initializer()
		return new LiveConstitution<Data>(data, this, changeSelf, nested) as LiveConstitution
	}
}
// deno-lint-ignore ban-types
export type OpaqueConstitution = Constitution<unknown, {}, {}, {}>

export class LiveConstitution<Data = unknown> {
	constructor(
		private data: Data,
		// deno-lint-ignore ban-types
		private readonly constitution: Constitution<Data, {}, {}, {}>,
		private readonly changeSelf: (newConstitution: LiveConstitution) => void,
		private readonly nested: Map<string, LiveConstitution>,
	) {}

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
			const nextLive = currentLive.nested.get(currentSegment)
			if (!nextLive) return errorMessage(`no view found at ${viewPath}`)
			currentLive = nextLive
		}

		// wish I could write a proof that this is true!
		throw new Error(`should be impossible to end maybeView after loop: ${viewPath}`)
	}

	private executeView(viewName: string, rawQuery: unknown) {
		return this.constitution.executeView(this.data, viewName, rawQuery)
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
			const nextLive = currentLive.nested.get(currentSegment)
			if (!nextLive) return errorMessage(`no action found at ${actionPath}`)
			currentLive = nextLive
		}

		// wish I could write a proof that this is true!
		throw new Error(`should be impossible to end maybeAction after loop: ${actionPath}`)
	}

	private executeAction(actionName: string, rawArg: unknown) {
		const result = this.constitution.executeAction(this.data, actionName, rawArg)
		if (Array.isArray(result)) return result

		if (result.change)
			// do the change
			this.changeSelf(result.next.makeLive(this.changeSelf))
		else
			// update the data
			this.data = result.data

		return undefined
	}
}
