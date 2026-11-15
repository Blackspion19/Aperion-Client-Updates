import type { AbstractAperionClient } from '@aperion/api-client'

import { createContext } from './create-context'

export const [injectAperionClient, provideAperionClient] = createContext<AbstractAperionClient>(
	'root',
	'aperionClient',
)
