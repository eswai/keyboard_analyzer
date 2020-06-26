<script>
	import Keyboard from './Keyboard.svelte';
	import mykeyboard from './jis_romaji.json';

	let text = "My best keyboard.";
</script>

<main>
	<h1>Keyboard Analyzer</h1>
	<textarea>{text}</textarea>
	<Keyboard layout={mykeyboard} />
</main>

<style>
	main {
		text-align: center;
		padding: 1em;
		max-width: 240px;
		margin: 0 auto;
	}

	h1 {
		color: #ff3e00;
		text-transform: uppercase;
		font-size: 4em;
		font-weight: 100;
	}

	@media (min-width: 640px) {
		main {
			max-width: none;
		}
	}
</style>