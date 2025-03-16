<template>
	<h1>Ruleset: {{ rulesetFullPath }}</h1>
	<NuxtLink :to="`/ruleset-detail/${rulesetFullPath}`">code details</NuxtLink>

	<div v-if="rulesetViews.loading">Loading views</div>
	<div v-else-if="rulesetViews.error" class="text-red-500">{{ displayError(rulesetViews.error) }}</div>

	<div v-else-if="rulesetViews.ok">
		<NuxtLink v-for="viewName in rulesetViews.value" :to="viewName">{{ viewName }}</NuxtLink>
		<!-- <NuxtLink v-for="viewName in rulesetViews.value" :to="`/rulesetViews/${rulesetFullPath}/${viewName}`">{{ viewName }}</NuxtLink> -->
	</div>

	<NuxtPage />
</template>

<script setup lang="ts">
import { asyncRulesetViews, displayError } from '@/utils/api'

const route = useRoute()

// TODO	get nuxt typed router
// https://nuxt-typed-router.vercel.app/guide
const rulesetFullPath = computed(() => route.params['full_path'] as string)

const rulesetViews = asyncRulesetViews(() => rulesetFullPath.value)

</script>
