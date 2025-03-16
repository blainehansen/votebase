<template>
	<h1>Ruleset: {{ rulesetFullPath }}</h1>

	<div v-if="rulesetDetail.loading">Loading ruleset details</div>
	<div v-else-if="rulesetDetail.error" class="text-red-500">{{ displayError(rulesetDetail.error) }}</div>

	<div v-else-if="rulesetDetail.ok">
		<h2>Code</h2>
		<pre v-if="rulesetDetail.value.code"><code>{{ rulesetDetail.value.code }}</code></pre>
		<div v-else>This ruleset has empty code.</div>

		<h2>Database schema</h2>
		<pre v-if="rulesetDetail.value.db_schema"><code>{{ rulesetDetail.value.db_schema }}</code></pre>
		<div v-else>This ruleset has an empty database schema.</div>

		<h2>Views</h2>
		<div v-if="rulesetDetail.value.views.length > 0">
			<NuxtLink v-for="viewName in rulesetDetail.value.views" :to="`/ruleset/${rulesetFullPath}/${viewName}`">{{ viewName }}</NuxtLink>
		</div>
		<div v-else>This ruleset has no views.</div>
	</div>
</template>

<script setup lang="ts">
import { asyncRulesetDetail, displayError } from '@/utils/api'

const route = useRoute()

// TODO	get nuxt typed router
// https://nuxt-typed-router.vercel.app/guide
const rulesetFullPath = computed(() => route.params['full_path'] as string)

const rulesetDetail = asyncRulesetDetail(() => rulesetFullPath.value)

</script>
