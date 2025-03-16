import tailwindcss from '@tailwindcss/vite'

// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
	compatibilityDate: '2024-11-01',
	devtools: { enabled: false },
	typescript: {
		typeCheck: true,
	},

	imports: {
		scan: false,
	},

	css: ['~/assets/main.css'],
	vite: {
		plugins: [
			tailwindcss(),
		],
	}
})
