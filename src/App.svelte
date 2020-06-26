<script>
	import Keyboard from './Keyboard.svelte';
	import mykeyboard from './jis_romaji.json';

	let text = "My best keyboard.";
	mykeyboard.rev = 0;

	function analyze() {
		for (let tk of mykeyboard.keys) {
			tk.value = 0;
		}
		let maxv = 0;
		for (let i = 0; i < text.length; i++) {
			// console.log(text.charAt(i));
			if (text.charAt(i) in mykeyboard.conversion) {
				let c = mykeyboard.conversion[text.charAt(i)]
				// console.log(c);
				for (let ck of c.keys) {
					for (let tk of mykeyboard.keys) {
						if (tk.id == ck) {
							tk.value++;
							if (maxv < tk.value) maxv = tk.value;
						}
					}
				}
			}
		}
		for (let tk of mykeyboard.keys) {
			tk.value /= maxv;
		}
		mykeyboard.rev++;
		// console.log(mykeyboard)
	}
</script>

<main>
	<h1>Keyboard Analyzer</h1>
	<textarea bind:value={text} />
	<button on:click={analyze}>Analyze</button>
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