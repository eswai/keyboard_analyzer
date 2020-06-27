<script>
	import Keyboard from './Keyboard.svelte';
	import mykeyboard from './jis_romaji.json';

	let text = "My best keyboard.";
	mykeyboard.rev = 0;

	let keydic = {};
	for (let mk of mykeyboard.keys) {
		keydic[mk.id] = mk;
	}

	let uncounted = 0;
	function analyze() {
		for (let tk of mykeyboard.keys) {
			tk.count = 0;
		}
		for (let i = 0; i < text.length; i++) {
			// console.log(text.charAt(i));
			if (text.charAt(i) in mykeyboard.conversion) {
				let mc = mykeyboard.conversion[text.charAt(i)]
				// console.log(c);
				for (let ck of mc.keys) {
					keydic[ck].count++;
				}
				for (let cs of mc.shift) {
					keydic[cs].count++;
				}
			} else {
				uncounted++;
			}
		}

		// normalize count
		let maxv = 0;
		for (let tk of mykeyboard.keys) {
			if (maxv < tk.count) maxv = tk.count;
		}
		for (let tk of mykeyboard.keys) {
			tk.value = tk.count / maxv;
		}
		mykeyboard.rev++;
		// console.log(mykeyboard)
	}
</script>

<main>
	<h1>Keyboard Analyzer</h1>
	<textarea bind:value={text} />
	<button on:click={analyze}>Analyze</button>
	<div class="kbd">
		<Keyboard layout={mykeyboard} />
	</div>
	<p>{uncounted} characters not counted.</p>
	
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
		font-size: 3em;
		font-weight: 100;
	}

	textarea {
		width: 100%;
	}

	.kbd {
		display: flex;
	}

	@media (min-width: 640px) {
		main {
			max-width: none;
		}
	}
</style>