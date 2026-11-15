<template>
	<div class="text-white">
		<svg class="rotate outer" width="100%" height="100%" viewBox="0 0 512 512" xmlns="http://www.w3.org/2000/svg">
			<circle cx="256" cy="256" r="246" fill="none" stroke="currentColor" stroke-width="10" stroke-linecap="round" stroke-dasharray="180 1366" />
		</svg>
		<svg class="pulse" width="100%" height="100%" viewBox="0 0 512 512" xmlns="http://www.w3.org/2000/svg">
			<g transform="translate(76 76) scale(0.703)">
				<path fill="currentColor" fill-rule="evenodd" d="M321.9 181.9L274.4 262.0L298.0 299.7L363.5 299.7L373.3 317.6L234.2 318.1L209.8 357.0L330.5 357.0L354.9 399.4L445.6 399.4ZM258.1 74.1L56.9 399.4L160.5 399.4L274.4 222.2L310.8 161.4Z" />
			</g>
		</svg>
	</div>
</template>

<style lang="scss" scoped>
div {
	position: relative;
	svg {
		position: absolute;
		inset: 0;
	}
}
.rotate {
	transform-origin: center;
	animation: spin 1.2s linear infinite;
}
.pulse {
	transform-origin: center;
	animation: pulse 1.6s ease-in-out infinite;
}
@keyframes spin {
	to {
		transform: rotate(360deg);
	}
}
@keyframes pulse {
	0%,
	100% {
		opacity: 1;
		transform: scale(1);
	}
	50% {
		opacity: 0.6;
		transform: scale(0.94);
	}
}
</style>
