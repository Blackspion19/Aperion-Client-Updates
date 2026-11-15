import type { ApiErrorData, AperionErrorResponse } from '../types/errors'
import { isAperionErrorResponse } from '../types/errors'

/**
 * Base error class for all Aperion API errors
 */
export class AperionApiError extends Error {
	/**
	 * HTTP status code (if available)
	 */
	readonly statusCode?: number

	/**
	 * Original error that was caught
	 */
	readonly originalError?: Error

	/**
	 * Response data from the API (if available)
	 */
	readonly responseData?: unknown

	/**
	 * Error context (e.g., module name, operation being performed)
	 */
	readonly context?: string

	constructor(message: string, data?: ApiErrorData) {
		super(message)
		this.name = 'AperionApiError'

		this.statusCode = data?.statusCode
		this.originalError = data?.originalError
		this.responseData = data?.responseData
		this.context = data?.context

		// Maintains proper stack trace for where our error was thrown (only available on V8)
		if (Error.captureStackTrace) {
			Error.captureStackTrace(this, AperionApiError)
		}
	}

	/**
	 * Create a AperionApiError from an unknown error
	 */
	static fromUnknown(error: unknown, context?: string): AperionApiError {
		if (error instanceof AperionApiError) {
			return error
		}

		if (error instanceof Error) {
			return new AperionApiError(error.message, {
				originalError: error,
				context,
			})
		}

		return new AperionApiError(String(error), { context })
	}
}

/**
 * Error class for Aperion server errors (kyros/archon)
 * Extends AperionApiError with V1 error response parsing
 */
export class AperionServerError extends AperionApiError {
	/**
	 * V1 error information (if available)
	 */
	readonly v1Error?: AperionErrorResponse

	constructor(message: string, data?: ApiErrorData & { v1Error?: AperionErrorResponse }) {
		// If we have a V1 error, format the message nicely
		let errorMessage = message
		if (data?.v1Error) {
			errorMessage = `[${data.v1Error.error}] ${data.v1Error.description}`
			if (data.v1Error.context) {
				errorMessage = `${data.v1Error.context}: ${errorMessage}`
			}
		}

		super(errorMessage, data)
		this.name = 'AperionServerError'
		this.v1Error = data?.v1Error

		if (Error.captureStackTrace) {
			Error.captureStackTrace(this, AperionServerError)
		}
	}

	/**
	 * Create a AperionServerError from response data
	 */
	static fromResponse(
		statusCode: number,
		responseData: unknown,
		context?: string,
	): AperionServerError {
		const v1Error = isAperionErrorResponse(responseData) ? responseData : undefined

		let message = `HTTP ${statusCode}`
		if (v1Error) {
			message = v1Error.description
		} else if (typeof responseData === 'string') {
			message = responseData
		}

		return new AperionServerError(message, {
			statusCode,
			responseData,
			context,
			v1Error,
		})
	}

	/**
	 * Create a AperionServerError from an unknown error
	 */
	static fromUnknown(error: unknown, context?: string): AperionServerError {
		if (error instanceof AperionServerError) {
			return error
		}

		if (error instanceof AperionApiError) {
			return new AperionServerError(error.message, {
				statusCode: error.statusCode,
				originalError: error.originalError,
				responseData: error.responseData,
				context: context ?? error.context,
			})
		}

		if (error instanceof Error) {
			return new AperionServerError(error.message, {
				originalError: error,
				context,
			})
		}

		return new AperionServerError(String(error), { context })
	}
}
