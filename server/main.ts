import { differenceInMilliseconds } from "date-fns"

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

// function nothing() { return new Response(undefined, { status: 204 }) }

// class Policy {}
// const policies = new Map<string, Policy>()

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
