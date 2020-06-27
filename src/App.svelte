<script>
	import Keyboard from './Keyboard.svelte';
	import mykeyboard from './jis_romaji.json';
	import kuromoji from './kuromoji/kuromoji.js';

	let text = "My best keyboard.";
	mykeyboard.rev = 0;

	let keydic = {};
	for (let mk of mykeyboard.keys) {
		keydic[mk.id] = mk;
	}

	function kanaToHira(str) {
    return str.replace(/[\u30a1-\u30f6]/g, function(match) {
        var chr = match.charCodeAt(0) - 0x60;
        return String.fromCharCode(chr);
    });
	}

	let uncounted = [];
	let ul = 0;
	function analyze() {
		let karray = [];
		uncounted = [];

		let hantext = text.replace(/[Ａ-Ｚａ-ｚ０-９]/g, function(s) {
			return String.fromCharCode(s.charCodeAt(0) - 65248);
		});

		kuromoji.builder({
			dicPath: 'dict'
		}).build((error, tokenizer) => {
			const parsed = tokenizer.tokenize(hantext);
			console.log(parsed);
			for (let pa of parsed) {
				if (pa.reading) {
					karray.push(kanaToHira(pa.reading));
				} else {
					karray.push(kanaToHira(pa.surface_form));
				}
			}
			
			let ktext = karray.join("");
			console.log(ktext);

			for (let tk of mykeyboard.keys) {
				tk.count = 0;
			}
			for (let i = 0; i < ktext.length; i++) {
				// console.log(ktext.charAt(i));
				let ch = ktext.charAt(i);
				if (ch in mykeyboard.conversion) {
					let mc = mykeyboard.conversion[ch]
					// console.log(c);
					for (let ck of mc.keys) {
						keydic[ck].count++;
					}
					for (let cs of mc.shift) {
						keydic[cs].count++;
					}
				} else {
					uncounted.push(ch);
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
			console.log(uncounted);
			ul = uncounted.length;
		
		});
	}
</script>

<main>
	<h1>Keyboard Analyzer</h1>
	<textarea bind:value={text} />
	<button on:click={analyze}>Analyze</button>
	<div class="kbd">
		<Keyboard layout={mykeyboard} />
	</div>
	<p>{ul} characters uncounted.</p>
	
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