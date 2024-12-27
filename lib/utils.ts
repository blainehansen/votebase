export type Dict<T> = { [key: string]: T }

export type NonEmpty<T> = [T, ...T[]]
export function isNonEmpty<T>(arr: T[]): arr is NonEmpty<T> {
	return arr.length !== 0
}
