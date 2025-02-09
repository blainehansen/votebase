type Dict<T> = { [key: string]: T }

export namespace JSX {
	// SVGElementTagNameMap
	export type IntrinsicElements = {
		// perhaps instead just reject functions?
		[K in keyof HTMLElementTagNameMap]:
			Partial<PickOfType<HTMLElementTagNameMap[K], string | number | bigint | boolean | null | undefined>>
			& { class?: string }
	}
	export type Element = RenderedNode
}

export type Comp<P extends Dict<unknown>> = (props: P & { children?: JSXNode | JSXNode[] | undefined }) => RenderedNode

export type JSXNode =
	| RenderedNode
	| RawContentNode
	| (() => JSXNode)
	| boolean
	| number
	| bigint
	| string
	| null
	| undefined

type KeysOfType<T, Condition> = {
	[K in keyof T]: T[K] extends Condition ? K : never
}[keyof T]

type PickOfType<T, Condition> = Pick<T, KeysOfType<T, Condition>>
type HTMLAttributes = Record<string, JSXNode | undefined> & JSXChildren
export interface JSXChildren {
	children?: JSXNode | JSXNode[] | undefined
}

interface RawContentNode {
	htmlContent: string
}

export type FunctionComponent = (props: Dict<unknown>) => RenderedNode



function renderAttributes(attributes: HTMLAttributes): string {
	let render = ""
	for (const attr of Object.entries(attributes)) {
		if (attr[0] === 'children') continue
		const value = serialize(attr[1], escapeProp)
		render += ` ${attr[0]}="${value}"`
	}
	return render
}

function renderChildren(attributes: HTMLAttributes): string {
	const children = attributes.children
	if (!children) return ""

	const childrenArray = !Array.isArray(children) ? [children] : children
	return childrenArray.map((c) => serialize(c, escapeHTML)).join("")
}

function renderTag(tag: string, attributes: string, children: string): string {
	const tagWithAttributes = [tag, attributes].join("")
	if (children.length !== 0)
		return `<${tagWithAttributes}>${children}</${tag}>`
	else
		return `<${tagWithAttributes}/>`
}

export function renderJSX(
	tag: string | FunctionComponent | undefined,
	props: HTMLAttributes,
	_key?: string
): JSX.Element {
	if (typeof tag === "function") {
		// handling for Function Components
		return tag(props)
	}
	else if (tag === undefined) {
		// handling for <></>
		return new RenderedNode(renderChildren(props))
	}
	else {
		// handling for plain HTML codes
		return new RenderedNode(
			renderTag(tag, renderAttributes(props), renderChildren(props))
		)
	}
}


export class RenderedNode {
	public constructor(public readonly value: string) {}
}

export const jsx = renderJSX
export const jsxs = renderJSX
export const jsxDEV = renderJSX



export function serialize(
	value: JSXNode,
	escaper: (value: string) => string
): string {
	if (value === null || value === undefined)
		return ""
	if (typeof value === "string")
		return escaper(value)
	if (typeof value === "number" || typeof value === "bigint")
		return value.toString()
	if (typeof value === "boolean")
		return value ? "true" : "false"

	if (typeof value === "function")
		return serialize(value(), escaper)

	if (value instanceof RenderedNode)
		return value.value

	return value.htmlContent
}

export function escapeProp(value: string): string {
	return value
		.replaceAll("&", "&amp;")
		.replaceAll('"', "&quot;")
		.replaceAll("<", "&lt;")
		.replaceAll(">", "&gt;")
		.replaceAll("\n", "&#10;")
		.trim()
}

export function escapeHTML(value: string): string {
	return value
		.replaceAll("&", "&amp;")
		.replaceAll('"', "&quot;")
		.replaceAll("'", "&#39;")
		.replaceAll("<", "&lt;")
		.replaceAll(">", "&gt;")
		.replaceAll("\n", "<br/>")
		.trim()
}
