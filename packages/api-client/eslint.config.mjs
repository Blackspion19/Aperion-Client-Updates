import config from '@aperion/tooling-config/eslint/nuxt.mjs'

export default config.append([
	{
		ignores: ['dist/'],
	},
])
