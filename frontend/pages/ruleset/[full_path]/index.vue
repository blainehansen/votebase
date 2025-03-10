<template>

		<!-- sandbox="allow-same-origin allow-scripts" -->
	<iframe
		v-if="currentView.ok"
		:srcdoc="currentView.value"
		ref="iframeRef"
		width="100%"
		@load="resize"
	/>
	<div v-else>Click on a ruleset view above.</div>

</template>

<script setup lang="ts">
import { asyncRuleset, asyncView } from '@/utils/api'

const route = useRoute()

// TODO do all of this with nested routing instead
// https://nuxt.com/docs/guide/directory-structure/pages#nested-routes

// TODO	get nuxt typed router
// https://nuxt-typed-router.vercel.app/guide
const ruleset = asyncRuleset(() => route.params['full_path'])

const currentViewName = ref<string | null>()
const currentView = asyncView(() => {
	return currentViewName.value
		? `/fn/view/${route.params['full_path']}|${currentViewName.value}`
		: undefined
})

const iframeRef = ref<HTMLIFrameElement | null>(null)


function setView(newViewName: string) {
	currentViewName.value = newViewName
}
function resize() {
	if (!iframeRef.value) return
	const iframe = iframeRef.value
	iframe.style.height = '0px' // reset height before measuring
	const height = iframe.contentDocument?.documentElement.scrollHeight
	if (height) {
		iframe.style.height = height + 'px'
	}
}

// import { asyncRulesets, displayError } from '@/utils/api'
// const rulesets = asyncRulesets()

</script>
