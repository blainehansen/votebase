<template>
	<h1>All rulesets</h1>

	<div v-if="rulesets.loading">Loading...</div>
	<div v-else-if="rulesets.error" class="text-red-500">Error? {{ displayError(rulesets.error) }}</div>

	<div v-else-if="rulesets.ok">
		<div v-if="rulesets.value.length > 0">
			<template v-for="ruleset in rulesets.value">
				<NuxtLink :to="`/ruleset/${ruleset.full_path}`">{{ ruleset.full_path }}</NuxtLink>
			</template>
		</div>

		<h2 v-else>You seem to have no rulesets?</h2>
	</div>
</template>

<script setup lang="ts">
import { asyncRulesetListings, displayError } from '@/utils/api'
const rulesets = asyncRulesetListings()
</script>
