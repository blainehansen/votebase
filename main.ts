import { isNonEmpty } from './lib/utils.ts'
import { differenceInMilliseconds } from 'date-fns'
import { demandEnv, errorMessage, pathnameToList } from './lib/serverUtils.ts'

import { LiveConstitution, Constitution } from './lib/index.ts'

function handleRequest(req: Request): Response {
	const url = new URL(req.url)
	const path = pathnameToList(url.pathname)
	if (!isNonEmpty(path))
		return errorMessage("no action or view specified")

	// if the request is a get, we're looking for a view
	if (req.method === 'GET') {
		const query = url.searchParams.get('query')
		return LiveConstitution.maybeView(constitutionRoot, path, query ? JSON.parse(decodeURIComponent(query)) : null)
	}

	// if it's a post, we're looking for an action
	if (req.method === 'POST') {
		const arg = url.searchParams.get('arg')
		return LiveConstitution.maybeAction(constitutionRoot, path, arg ? JSON.parse(decodeURIComponent(arg)) : null)
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
	return new Constitution(
		() => null,
		() => [],
		{},
		{},
		{},
	).makeLive(c => { constitutionRoot = c })
}

let constitutionRoot = fetchRootConstitution()

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
