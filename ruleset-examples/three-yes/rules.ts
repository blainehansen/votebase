import * as z from 'zod'

declare function registerAction<I, O>(name: string, input: z.ZodSchema<I>, output: z.ZodSchema<O>, func: (input: I) => O): void
declare function registerView<Q>(name: string, input: z.ZodSchema<Q>, func: (query: Q) => void): void

registerAction('nominateConstitution', z.string(), z.string().uuid(), (text) => {
	//
})

const YesOrNo = z.strictObject({
	isYes: z.boolean(),
	constitutionId: z.string().uuid(),
})
type YesOrNo = z.TypeOf<typeof YesOrNo>

registerAction('yesOrNo', YesOrNo, z.void(), ({ constitutionId, isYes }) => {
	//
})
