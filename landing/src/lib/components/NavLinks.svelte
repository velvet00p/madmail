<script>
	/** @type {{ links: { label: string, href: string }[], class?: string, currentHref?: string, ariaLabel?: string }} */
	let { links, class: className = '', currentHref = '', ariaLabel = 'Documentation' } = $props();

	/** @param {string} href */
	function isExternal(href) {
		return href.startsWith('http://') || href.startsWith('https://');
	}
</script>

<nav class={className} aria-label={ariaLabel}>
	{#each links as link}
		<a
			href={link.href}
			class:active={currentHref === link.href}
			aria-current={currentHref === link.href ? 'page' : undefined}
			rel={isExternal(link.href) ? 'noopener noreferrer' : undefined}
		>
			{link.label}
		</a>
	{/each}
</nav>
