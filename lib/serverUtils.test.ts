import { assertEquals } from "@std/assert"

import { pathnameToList } from './serverUtils.ts'

for (const [url, expected] of [
	['/', []],
	['/  ', []],
	['/a/b/c', ['a', 'b', 'c']],
	['/a/b/c/', ['a', 'b', 'c']],

	['/a/b/c?asdf=44', ['a', 'b', 'c']],
	['/a/b/c#asdf', ['a', 'b', 'c']],
] as [string, string[]][]) {
	Deno.test(`pathnameToList ${url}`, () => {
		assertEquals(pathnameToList(new URL('http://.' + url).pathname), expected)
	})
}

