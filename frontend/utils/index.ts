import type { ShallowRef } from 'vue'
import type { ZodSchema, SafeParseReturnType, ZodError } from 'zod'

export type Result<T, E> =
	| { ok: true, value: T, error?: never }
	| { ok?: never, value?: never, error: E }

export type ZodResult<T, E = never> = Result<T, ZodError<unknown> | E>

export type Async<T, E> =
	| { init: true, ok?: never, value?: never, loading?: never, error?: never }
	| { init?: never, ok?: never, value?: never, loading: true, error?: never }
	| { init?: never, ok: true, value: T, loading?: never, error?: never }
	| { init?: never, ok?: never, value?: never, loading?: never, error: E }

export namespace Async {
	export type AsyncRef<T, E> = Readonly<ShallowRef<Readonly<Async<T, E>>>> & { readonly refresh: () => void }

	export function create<T, E>(fn: () => Promise<Result<T, E>>, lazy = false) {
		const inner = shallowRef<Async<T, E>>({ init: true })

		async function refresh() {
			inner.value = { loading: true }
			const response = await fn()
			inner.value = response.ok
				? { ok: true, value: response.value }
				: { error: response.error }
		}
		(inner as any).refresh = refresh

		if (!lazy) onMounted(refresh)

		return inner as AsyncRef<T, E>
	}

	export function fetch<T>(...args: Parameters<typeof safeFetch<T>>) {
		return create(() => safeFetch(...args))
	}

	export type Infallible<T> = Result<T, never>
	export function infallible<T>(fn: () => Promise<T>) {
		return create(async (): Promise<Infallible<T>> => ({ ok: true, value: await fn() }))
	}
}



// TODO get proper server prefix
// https://nuxt.com/docs/guide/going-further/runtime-config#environment-variables
const SERVER_PREFIX = 'http://localhost:8080'

export async function safeFetch<T>(parser: ZodSchema<T>, ...args: Parameters<typeof fetch>): Promise<ZodResult<T, string>> {
	try {
		const response = await fetch(...args)
		if (!response.ok) return { error: response.statusText }
		const data: unknown = await (response).json()
		const parsed = parser.safeParse(data)
		return parsed.success ? { ok: true, value: parsed.data } : { error: parsed.error }
	}
	catch (e) {
		return { error: `${e}` }
	}
}
