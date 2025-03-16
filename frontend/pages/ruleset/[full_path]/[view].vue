<template>

	<div v-if="viewTemplate.loading">Loading view</div>
	<div v-else-if="viewTemplate.error" class="text-red-500">{{ displayError(viewTemplate.error) }}</div>

		<!-- sandbox="allow-same-origin allow-scripts" -->
	<iframe
		v-else-if="viewTemplate.ok"
		:srcdoc="viewTemplate.value"
		ref="iframeRef"
		width="100%"
		@load="resize"
	/>

</template>

<script setup lang="ts">
import { asyncViewTemplate, displayError } from '@/utils/api'

const route = useRoute()

// TODO	get nuxt typed router
// https://nuxt-typed-router.vercel.app/guide
const viewName = computed(() => route.params['view'] as string)

const viewTemplate = asyncViewTemplate(() => viewName.value)

const iframeRef = ref<HTMLIFrameElement | null>(null)

function resize() {
	if (!iframeRef.value) return
	const iframe = iframeRef.value
	iframe.style.height = '0px' // reset height before measuring
	const height = iframe.contentDocument?.documentElement.scrollHeight
	if (height)
		iframe.style.height = height + 'px'
}

</script>
