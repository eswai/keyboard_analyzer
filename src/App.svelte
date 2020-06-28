<script>
  import Keyboard from './Keyboard.svelte';
  import romaji from './keyboards/jis_romaji.json';
  import naginata from './keyboards/jis_naginata.json';
  import kuromoji from './kuromoji/kuromoji.js';
  import Chart from 'svelte-frappe-charts';

  const keyboards = {
    "ローマ字" : romaji,
    "薙刀式": naginata
  };

  let selected_kb = "ローマ字";
  let mykeyboard = keyboards[selected_kb];

  let text = "人類が増えすぎた人口を宇宙に移民させるようになって、既に半世紀が過ぎていた。地球の周りの巨大な人工都市は人類の第二の故郷となり、人々はそこで子を産み、育て、そして死んでいった。";

  let uncounted = []; // 入力できなかった文字
  let ul = 0; // 入力できなかった文字数
  let total_char = 0; // 入力した文字数
  let total_key = 0; // 打鍵したキー数
  let total_kana = 0; // 入力した文字数（かな）
  let finger_count = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  let total_action = 0;
  let showkb = false;
  let finger_chart;

  let keydic = {};
  $: for (let mk of mykeyboard.keys) {
    keydic[mk.id] = mk;
  }

  function kanaToHira(str) {
    return str.replace(/[\u30a1-\u30f6]/g, function(match) {
        let chr = match.charCodeAt(0) - 0x60;
        return String.fromCharCode(chr);
    });
  }

  function incCounter(c) {
    let mc = mykeyboard.conversion[c]
    // console.log(c);
    if (mc.type == "sim") {
      total_action++;
    } else {
      total_action += mc.keys.length + mc.shift.length;
    }
    for (let ck of mc.keys) {
			keydic[ck].count++;
      total_key++;
      finger_count[keydic[ck].finger]++;
    }
    for (let cs of mc.shift) {
			keydic[cs].count++;
			total_key++;
      finger_count[keydic[cs].finger]++;
    }
  }

  function analyze() {
    showkb = false;
    let karray = []; // 変換後のかな文字の配列
    uncounted = [];
    total_char = text.length;
    total_key = 0;
    finger_count = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    total_action = 0;

    mykeyboard = keyboards[selected_kb];

    // 全角英数を半角に変換
    let hantext = text.replace(/[Ａ-Ｚａ-ｚ０-９]/g, function(s) {
      return String.fromCharCode(s.charCodeAt(0) - 65248);
    });

    kuromoji.builder({
      dicPath: 'dict' // public/dict
    }).build((error, tokenizer) => {
      // 形態素解析
      const parsed = tokenizer.tokenize(hantext);
      // console.log(parsed);
      for (let pa of parsed) {
        if (pa.reading) { // 漢字、カナ
          karray.push(kanaToHira(pa.reading));
        } else { // 英数字
          karray.push(kanaToHira(pa.surface_form));
        }
      }

      let ktext = karray.join("");
      total_kana = ktext.length;
      console.log(ktext);

      for (let tk of mykeyboard.keys) {
        tk.count = 0;
      }
      for (let i = 0; i < ktext.length; i++) {
        // console.log(ktext.charAt(i));
        let ch1 = ktext.charAt(i);
        let ch2 = ktext.substr(i, 2);
        if (ch2 in mykeyboard.conversion) {
          incCounter(ch2);
          i++;
        } else if (ch1 in mykeyboard.conversion) {
          incCounter(ch1)
        } else {
          uncounted.push(ch1);
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
      mykeyboard.rev = Math.random();
      // console.log(mykeyboard)
      console.log(uncounted);
      ul = uncounted.length;

      console.log(finger_count);
      finger_chart = {
        labels: ['左小', '左薬', '左中', '左人', '左親', '右親', '右人', '右中', '右薬', '右小'],
        datasets: [
          {
            values: finger_count,
          }
        ]
      };
      showkb = true;
    
    });
  }
</script>

<main>
  <h1>keyboard layout analyzer</h1>

  <textarea bind:value={text} />

  <select bind:value={selected_kb}>
    {#each Object.keys(keyboards) as k}
    <option>{k}</option>
    {/each}
  </select>

  <button on:click={analyze}>Analyze</button>

  {#if showkb == true}
  <div class="kbd">
    <Keyboard layout={mykeyboard} />
  </div>

  <p class="info">入力した文字数 {total_char}</p>
  <p class="info">入力した文字数(かな) {total_kana}</p>
  <p class="info">入力できなかった文字数 {ul}</p>
  <p class="info">打鍵したキー数 {total_key}</p>
  <p class="info">打鍵アクション数 {total_action}</p>

  <div class="chart">
    <Chart data={finger_chart} type="bar" height="200" />
  </div>
  {/if}

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

  .info {
    font-size: 10pt;
  }

  .chart {
    width: 600px;
  }

  @media (min-width: 640px) {
    main {
      max-width: none;
    }
  }
</style>