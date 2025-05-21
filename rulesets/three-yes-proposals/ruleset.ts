import 'votebase'

// votebase.Action('nominateConstitution', z.string(), z.string().uuid(), (text) => {
votebase.Action('nominateConstitution', async (text) => {
	//
})

// const YesOrNo = z.strictObject({
// 	isYes: z.boolean(),
// 	constitutionId: z.string().uuid(),
// })
// type YesOrNo = z.TypeOf<typeof YesOrNo>

// votebase.Action('yesOrNo', YesOrNo, z.void(), ({ constitutionId, isYes }) => {
votebase.Action('yesOrNo', async ({ constitutionId, isYes }) => {
	//
})
