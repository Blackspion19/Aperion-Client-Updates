import type { AbstractAperionClient } from '@aperion/api-client'
import { provideUserCountry } from '@aperion/ui'
import { ref } from 'vue'

export function setupUserCountryProvider(client: AbstractAperionClient) {
	const country = ref('US')

	void client.labrinth.geoip
		.getCountry()
		.then((detectedCountry) => {
			country.value = detectedCountry ?? country.value
		})
		.catch(() => {})

	return provideUserCountry(country)
}
