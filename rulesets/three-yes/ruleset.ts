import 'votebase'
import * as z from 'zod'

// votebase.registerAction('nominateConstitution', z.string(), z.string().uuid(), (text) => {
votebase.registerAction('nominateConstitution', (text) => {
	//
})

const YesOrNo = z.strictObject({
	isYes: z.boolean(),
	constitutionId: z.string().uuid(),
})
type YesOrNo = z.TypeOf<typeof YesOrNo>

// votebase.registerAction('yesOrNo', YesOrNo, z.void(), ({ constitutionId, isYes }) => {
votebase.registerAction('yesOrNo', ({ constitutionId, isYes }) => {
	//
})
