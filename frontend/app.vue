<template>
	<div></div>

	<div v-if="views.length > 0">
		<button
			class="cursor-pointer hover:bg-sky-200 block p-1 border border-gray-500 rounded-md"
			v-for="view in views" @click="setView(view.doc)"
		>
			{{ view.name }}
		</button>
	</div>

	<div></div>

		<!-- sandbox="allow-same-origin allow-scripts" -->
	<iframe 
		v-if="fetchedView" 
		:srcdoc="fetchedView"
		ref="iframeRef"
		width="100%"
		@load="resize"
	/>
	<div v-else>Click on a ruleset view above.</div>

</template>

<script setup lang="ts">
const iframeRef = ref<HTMLIFrameElement | null>(null)
const fetchedView = ref<string | null>(null)


function setView(newDoc: string) {
	fetchedView.value = newDoc
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

import { Async } from './utils/index'

// const views = Async.fetch()

// const views = [
// 	{ name: 'red', doc: Array.from({ length: 50 }, () => '<p>red</p>').join('') },
// 	{ name: 'green', doc: Array.from({ length: 50 }, () => '<p>green</p>').join('') },
// 	{ name: 'blue', doc: Array.from({ length: 50 }, () => '<p>blue</p>').join('') },
// ]

</script>
