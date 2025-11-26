// TODO remove if you have no queries
import queries from './queries'

// docs for how to write a ruleset:
// TODO blaine

votebase.Action('insertFruit', async (fruitName: string, memberId) => {
	const newFruitId = await queries.insertFruit(fruitName, memberId)
	console.log(newFruitId)
})

votebase.View('allFruit', async (_: unknown, _memberId) => {
	const allFruit = await queries.allFruit.fetchAll()
	return JSON.stringify(allFruit)
})

votebase.View('oneFruit', async (fruitId: string, _memberId) => {
	const oneFruit = await queries.oneFruit.fetchOptional(fruitId)
	return JSON.stringify(oneFruit)
})

