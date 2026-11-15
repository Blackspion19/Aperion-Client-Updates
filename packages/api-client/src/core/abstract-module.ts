import type { AbstractAperionClient } from './abstract-client'

export abstract class AbstractModule {
	protected client: AbstractAperionClient

	public constructor(client: AbstractAperionClient) {
		this.client = client
	}

	/**
	 * Get the module's name, used for error reporting & for module field generation.
	 * @returns Module name
	 */
	public abstract getModuleID(): string
}
