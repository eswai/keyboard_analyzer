<script>
  import Keyboard from './Keyboard.svelte';
  import romaji from './keyboards/jis_romaji.json';
  import naginata from './keyboards/jis_naginata.json';
  import nicola from './keyboards/jis_nicola.json';
  import kuromoji from './kuromoji/kuromoji.js';
  import Chart from 'svelte-frappe-charts';

  const keyboards = {
    "ローマ字" : romaji,
    "薙刀式": naginata,
    "親指シフト": nicola
  };

  let selected_kb = "ローマ字";
  let mykeyboard = keyboards[selected_kb];

  let text = "人類が増えすぎた人口を宇宙に移民させるようになって、既に半世紀が過ぎていた。地球の周りの巨大な人工都市は人類の第二の故郷となり、人々はそこで子を産み、育て、そして死んでいった。";

  let uncounted = []; // 入力できなかった文字
  let ul = 0; // 入力できなかった文字数
  let total_char = 0; // 入力した文字数
  let total_key = 0; // 打鍵したキー数
  let total_skey = 0; // 打鍵したキー数、連続シフトを考慮
  let total_kana = 0; // 入力した文字数（かな）
  let finger_count = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  let total_action = 0;
  let same_finger = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  let showkb = false;
  let finger_chart;
  let samefinger_chart;
  let arpeggio_chart;
  let last_key = ""; // 直前に押したキー
  let total_arpeggio;
  let total_alter = 0; // 交互打鍵

  let keydic = {};
  $: for (let mk of mykeyboard.keys) {
    keydic[mk.id] = mk;
  }

  var sum  = function(arr) {
      return arr.reduce(function(prev, current, i, arr) {
          return prev+current;
      });
  };

  function kanaToHira(str) {
    return str.replace(/[\u30a1-\u30f6]/g, function(match) {
        let chr = match.charCodeAt(0) - 0x60;
        return String.fromCharCode(chr);
    });
  }

  let shift_key = [];
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
      total_skey++;
      finger_count[keydic[ck].finger]++;
      // 同じ指で違うキーを連続して押す
      if (ck in keydic && last_key in keydic && ck != last_key && keydic[ck].finger == keydic[last_key].finger) {
        same_finger[keydic[ck].finger]++;
      }
      // アルペジオ
      for (let i = 0; i < mykeyboard.arpeggio.length; i++) {
        let ar = mykeyboard.arpeggio[i];
        if (ar.includes(ck) && ar.includes(last_key)) {
          total_arpeggio[i]++;
        }
      }
      // 交互打鍵
      if (ck in keydic && last_key in keydic && ((keydic[ck].finger < 5 && keydic[last_key].finger >= 5) || (keydic[ck].finger >= 5 && keydic[last_key].finger < 5))) {
        total_alter++;
      }
      last_key = ck;
    }
    if (mc.shift.length == 0) {
      shift_key = [];
    } else {
      for (let cs of mc.shift) {
        // 連続シフト
        if (!shift_key.includes(cs)) {
          total_skey++;
        }
        keydic[cs].count++;
        total_key++;
        finger_count[keydic[cs].finger]++;
      }
      shift_key = mc.shift;
    }

  }

  function analyze() {
    mykeyboard = keyboards[selected_kb];

    showkb = false;
    let karray = []; // 変換後のかな文字の配列
    uncounted = [];
    total_char = text.length;
    total_key = 0;
    total_skey = 0;
    finger_count = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    total_action = 0;
    shift_key = [];
    same_finger = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    last_key = "";
    total_arpeggio = new Array(mykeyboard.arpeggio.length);
    total_arpeggio.fill(0);
    total_alter = 0;

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

      // console.log(finger_count);
      finger_chart = {
        labels: ['左小', '左薬', '左中', '左人', '左親', '右親', '右人', '右中', '右薬', '右小'],
        datasets: [
          {
            values: finger_count
          }
        ]
      };
      samefinger_chart = {
        labels: ['左小', '左薬', '左中', '左人', '左親', '右親', '右人', '右中', '右薬', '右小'],
        datasets: [
          {
            values: same_finger
          }
        ]
      };
      arpeggio_chart = {
        labels: mykeyboard.arpeggio,
        datasets: [
          {
            values: total_arpeggio
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
  <p class="info">入力した文字数 {total_char}</p>
  <p class="info">入力した文字数(かな) {total_kana}</p>
  <p class="info">入力できなかった文字数 {ul}</p>
  <p class="info">打鍵したキー数 {total_key}</p>
  <p class="info">連続シフトした場合の打鍵キー数 {total_skey}</p>
  <p class="info">打鍵アクション数 {total_action}</p>
  <p class="info">アルペジオの数 {sum(total_arpeggio)}</p>
  <p class="info">交互打鍵の数 {total_alter}</p>
  <p class="info">同じ指で連続して違うキーを打鍵した数 {sum(same_finger)}</p>

  <label class="chart">キー打鍵ヒートマップ</label>
  <div class="kbd">
    <Keyboard layout={mykeyboard} />
  </div>

  <div class="chart">
    指ごとの打鍵数
    <Chart data={finger_chart} type="bar" height="200" />
  </div>

  <div class="chart">
    同じ指で連続して違うキーを打鍵した数
    <Chart data={samefinger_chart} type="bar" height="200" />
  </div>

  <div class="chart">
    アルペジオの詳細
    <Chart data={arpeggio_chart} type="bar" height="200" />
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
    margin: 10px auto;
  }

  .info {
    font-size: 10pt;
  }

  .chart {
    font-size: 10pt;
    width: 600px;
    margin: 0px auto;
  }

  @media (min-width: 640px) {
    main {
      max-width: none;
    }
  }
</style>