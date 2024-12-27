export function demandEnv(variable: string) {
	const v = Deno.env.get(variable)
	if (!v) throw new Error(`env variable ${variable} must be set`)
	return v
}

export function json(body: object, { status = 200, headers = {}, ...rest }: ResponseInit = {}): Response {
	return new Response(JSON.stringify(body), {
		...rest,
		status,
		headers: {
			"content-type": "application/json; charset=utf-8",
			...headers,
		},
	})
}

export function errorMessage(message: string, { status = 400, headers = {}, ...rest }: ResponseInit = {}): Response {
	return new Response(message, {
		...rest,
		status,
		headers: {
			"content-type": "application/json; charset=utf-8",
			...headers,
		},
	})
}

export function noContent() { return new Response(undefined, { status: 204 }) }

export function pathnameToList(pathname: string) {
	if (pathname === '/') return []

	const endIndex = pathname.endsWith('/') ? pathname.length - 1 : undefined
	return pathname.slice(1, endIndex).split('/')
}
