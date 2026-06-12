<script>
	import { activateMadMode } from '$lib/madMode.svelte.js';
	import logo from '$lib/logoUrl.js';

	/** @type {{ size?: string, class?: string, alt?: string, interactive?: boolean, href?: string, transitionName?: string }} */
	let {
		size = 'min(72vw, 14rem)',
		class: className = '',
		alt = 'Madmail',
		interactive = false,
		href = '',
		transitionName = ''
	} = $props();

	const transitionStyle = $derived(
		transitionName ? `view-transition-name: ${transitionName};` : ''
	);

	const logoImageStyle = $derived(`--mad-logo-image: url("${logo}");`);

	function handleClick() {
		activateMadMode();
	}
</script>

{#snippet logoMarkup(showAlt)}
	<div class="mad-logo__glitch" aria-hidden="true">
		<div class="mad-logo__layer mad-logo__layer--red"></div>
		<div class="mad-logo__layer mad-logo__layer--cyan"></div>
	</div>
	<div class="mad-logo__main" aria-hidden={showAlt ? undefined : true}></div>
{/snippet}

{#if href}
	<a
		{href}
		class="mad-logo mad-logo--link {className}"
		style="--mad-logo-size: {size}; {logoImageStyle} {transitionStyle}"
		aria-label={alt}
	>
		{@render logoMarkup(false)}
	</a>
{:else if interactive}
	<button
		type="button"
		class="mad-logo mad-logo--interactive {className}"
		style="--mad-logo-size: {size}; {logoImageStyle} {transitionStyle}"
		aria-label={alt}
		onclick={handleClick}
	>
		{@render logoMarkup(false)}
	</button>
{:else}
	<div
		class="mad-logo {className}"
		style="--mad-logo-size: {size}; {logoImageStyle} {transitionStyle}"
		role="img"
		aria-label={alt}
	>
		{@render logoMarkup(true)}
	</div>
{/if}

<style>
	.mad-logo {
		position: relative;
		width: var(--mad-logo-size);
		aspect-ratio: 1;
	}

	.mad-logo--interactive,
	.mad-logo--link {
		display: block;
		padding: 0;
		border: none;
		background: none;
		color: inherit;
		text-decoration: none;
		cursor: pointer;
	}

	.mad-logo--interactive {
		transition: transform 0.12s cubic-bezier(0.4, 0, 0.2, 1);
		transform-origin: center center;
	}

	.mad-logo--interactive:active {
		transform: scale(0.88);
	}

	.mad-logo__glitch {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}

	.mad-logo__layer,
	.mad-logo__main {
		width: 100%;
		height: 100%;
		background-image: var(--mad-logo-image);
		background-repeat: no-repeat;
		background-position: center;
		background-size: contain;
		user-select: none;
		-webkit-user-drag: none;
	}

	.mad-logo__layer {
		position: absolute;
		inset: 0;
	}

	.mad-logo__layer--red {
		opacity: 0;
		mix-blend-mode: multiply;
		filter: sepia(1) saturate(8) hue-rotate(-50deg);
		animation: mad-logo-glitch-red 3.2s infinite steps(1);
	}

	.mad-logo__layer--cyan {
		opacity: 0;
		mix-blend-mode: multiply;
		filter: sepia(1) saturate(6) hue-rotate(140deg);
		animation: mad-logo-glitch-cyan 3.2s infinite steps(1);
	}

	.mad-logo__main {
		position: relative;
		z-index: 1;
		animation: mad-logo-main-flicker 3.2s infinite steps(1);
	}

	@keyframes mad-logo-main-flicker {
		0%,
		88%,
		100% {
			transform: translate(0);
		}

		89% {
			transform: translate(-2px, 1px) skewX(-1deg);
		}

		90% {
			transform: translate(3px, -2px) skewX(1.5deg);
		}

		91% {
			transform: translate(-1px, 0);
		}

		93% {
			transform: translate(2px, 2px);
		}

		94% {
			transform: translate(0);
		}
	}

	@keyframes mad-logo-glitch-red {
		0%,
		86%,
		100% {
			opacity: 0;
			transform: translate(0);
			clip-path: inset(0 0 0 0);
		}

		87% {
			opacity: 0.85;
			transform: translate(-5px, 2px);
			clip-path: inset(12% 0 58% 0);
		}

		88% {
			opacity: 0.7;
			transform: translate(4px, -3px);
			clip-path: inset(48% 0 22% 0);
		}

		89% {
			opacity: 0.9;
			transform: translate(-3px, 1px);
			clip-path: inset(72% 0 8% 0);
		}

		90% {
			opacity: 0;
		}

		95% {
			opacity: 0.75;
			transform: translate(6px, 0);
			clip-path: inset(30% 0 45% 0);
		}

		96% {
			opacity: 0;
		}
	}

	@keyframes mad-logo-glitch-cyan {
		0%,
		84%,
		100% {
			opacity: 0;
			transform: translate(0);
			clip-path: inset(0 0 0 0);
		}

		85% {
			opacity: 0.8;
			transform: translate(5px, -2px);
			clip-path: inset(8% 0 62% 0);
		}

		86% {
			opacity: 0.65;
			transform: translate(-4px, 3px);
			clip-path: inset(55% 0 18% 0);
		}

		87% {
			opacity: 0.85;
			transform: translate(3px, -1px);
			clip-path: inset(68% 0 12% 0);
		}

		88% {
			opacity: 0;
		}

		94% {
			opacity: 0.7;
			transform: translate(-6px, 1px);
			clip-path: inset(25% 0 50% 0);
		}

		95% {
			opacity: 0;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.mad-logo--interactive {
			transition: none;
		}

		.mad-logo__layer--red,
		.mad-logo__layer--cyan,
		.mad-logo__main {
			animation: none;
		}
	}
</style>
