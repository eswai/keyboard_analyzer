<script>
  import Keyboard from './Keyboard.svelte';
  import romaji from './keyboards/jis_romaji.json';
  import naginata from './keyboards/jis_naginata.json';
  import nicola from './keyboards/jis_nicola.json';
  import eucalyn from './keyboards/jis_eucalyn.json';
  import shingeta from './keyboards/jis_shingeta.json';
  import asuka from './keyboards/jis_asuka123.json';
  import kuromoji from './kuromoji/kuromoji.js';
  import Chart from 'svelte-frappe-charts';

  import Textfield from '@smui/textfield';
  import Button, {Label} from '@smui/button';
  import Dialog, {Title, Content, Actions, InitialFocus} from '@smui/dialog';
  import Checkbox from '@smui/checkbox';
  import FormField from '@smui/form-field';
  import Select, {Option} from '@smui/select';
  import DataTable, {Head, Body, Row, Cell} from '@smui/data-table';
  import Card from '@smui/card';

  const keyboards = {
    "QWERTYローマ字" : romaji,
    "薙刀式": naginata,
    "親指シフト": nicola,
    "Eucalynローマ字": eucalyn,
    "新下駄": shingeta,
    "飛鳥123": asuka
  };

  let selected_kb = "QWERTYローマ字";
  let mykeyboard = keyboards[selected_kb];

  let text = "人類が増えすぎた人口を宇宙に移民させるようになって、既に半世紀が過ぎていた。地球の周りの巨大な人工都市は人類の第二の故郷となり、人々はそこで子を産み、育て、そして死んでいった。";

  let uncounted = []; // 入力できなかった文字
  let ul = 0; // 入力できなかった文字数
  let total_char = 0; // 入力した文字数
  let total_key = 0; // 打鍵したキー数
  let total_schar = 0; // 打鍵したキー数、連続シフトを考慮
  let total_kana = 0; // 入力した文字数（かな）
  let renzoku_shift = 0;
  let finger_tandoku= [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  let finger_douji = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  let finger_shifted = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  let total_action = 0;
  let same_finger = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  let showkb;
  let finger_chart;
  let samefinger_chart;
  let arpeggio_chart;
  let last_key = ""; // 直前に押したキー
  let total_arpeggio;
  let total_alter = 0; // 交互打鍵
  let kana_only = false;
  let aozora = false;
  let keyseq = "";

  let optionDialog;
  let remarkDialog;
  let remark = "";

  let keydic = {};
  $: for (let mk of mykeyboard.keys) {
    keydic[mk.id] = mk;
  }

  var sum = function(arr) {
      return arr.reduce(function(prev, current, i, arr) {
          return prev+current;
      });
  };

  function percent(v) {
    return (v * 100).toFixed(1) + '%'
  }

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
    if (mc.shift.length > 0) {
      total_schar++;
    }

    for (let ck of mc.keys) {
      if (mc.shift.length > 0) {
        keydic[ck].shifted++;
      } else if (mc.type == 'sim' && mc.keys.length > 1) {
        keydic[ck].douji++;
      } else {
        keydic[ck].tandoku++;
      }
      
      keydic[ck].count++;
      total_key++;
      keyseq += ck;

      if (ck in keydic && last_key in keydic) {
        // 同じ指で違うキーを連続して押す
        if (ck != last_key && keydic[ck].finger == keydic[last_key].finger) {
          same_finger[keydic[ck].finger]++;
        }
        // アルペジオ
        for (let i = 0; i < mykeyboard.arpeggio.length; i++) {
          let ar = mykeyboard.arpeggio[i];
          if ((ar[0] == ck && ar[1] == last_key) || (ar[1] == ck && ar[0] == last_key)) {
            total_arpeggio[i]++;
            // console.log(last_key, ck);
          }
        }
        // 交互打鍵
        if (((keydic[ck].finger < 5 && keydic[last_key].finger >= 5) || (keydic[ck].finger >= 5 && keydic[last_key].finger < 5))) {
          total_alter++;
        }
      }
      last_key = ck;
    }
    if (mc.shift.length == 0) {
      shift_key = [];
    } else {
      for (let cs of mc.shift) {
        // 連続シフト
        if (mc.renzsft && shift_key.includes(cs)) {
          renzoku_shift++;
        } else {
          keydic[cs].count++;
          total_key++;
          keydic[cs].shifted++;
        }
      }
      shift_key = mc.shift;
    }

  }

  function kbchange() {
    remark = keyboards[selected_kb].remark;
  }

  function analyze() {
    mykeyboard = keyboards[selected_kb];

    showkb = false;
    let karray = []; // 変換後のかな文字の配列
    uncounted = [];
    total_char = text.length;
    total_key = 0;
    total_schar = 0;
    renzoku_shift = 0;
    finger_tandoku = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    finger_douji = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    finger_shifted = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    total_action = 0;
    shift_key = [];
    same_finger = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    last_key = "";
    total_arpeggio = new Array(mykeyboard.arpeggio.length);
    total_arpeggio.fill(0);
    total_alter = 0;
    keyseq = "";

    // 全角英数を半角に変換
    let hantext = text.replace(/[＂-＇＊-＞＠-ｚ]/g, function(s) {
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
      if (kana_only) {
        ktext = ktext.replace(/[\u0000-\u2999\u3003-\u3040\u3097-\uff00\uff02-\uff1e\uff20-\uffff]/g, "");
      }
      if (aozora) {
        ktext = ktext.replace(/（.+?）/g, "");
        ktext = ktext.replace(/《.+?》/g, "");
        ktext = ktext.replace(/｜/g, "");
      }

      total_kana = ktext.length;
      console.log(ktext);

      for (let tk of mykeyboard.keys) {
        tk.count = 0; // 合計
        tk.tandoku = 0; // 単独
        tk.douji = 0; // シフトではない同時押し
        tk.shifted = 0; // シフト入力
      }
      for (let i = 0; i < ktext.length; i++) {
        // console.log(ktext.charAt(i));
        let ch1 = ktext.charAt(i);
        let ch2 = ktext.substr(i, 2);
        let ch3 = ktext.substr(i, 3);
        if (ch3 in mykeyboard.conversion) {
          incCounter(ch3);
          i += 2;
        } else if (ch2 in mykeyboard.conversion) {
          incCounter(ch2);
          i++;
        } else if (ch1 in mykeyboard.conversion) {
          incCounter(ch1)
        } else {
          uncounted.push(ch1);
        }
      }

      for (let k of mykeyboard.keys) {
        finger_tandoku[k.finger] += k.tandoku;
        finger_douji[k.finger] += k.douji;
        finger_shifted[k.finger] += k.shifted;
      }

      console.log(keyseq);
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
            name: "単独",
            values: finger_tandoku
          },
          {
            name: "同時",
            values: finger_douji
          },
          {
            name: "シフト",
            values: finger_shifted
          },
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

  <Textfield fullwidth textarea bind:value={text} label="入力テキスト" />

  <div class="inputfield">
    <Select bind:value={selected_kb} label="配列">
      {#each Object.keys(keyboards) as k}
      <option>{k}</option>
      {/each}
    </Select>
  
  <Button color="secondary" on:click={analyze} variant="outlined"><Label>分析開始</Label></Button>

  <Dialog bind:this={optionDialog} aria-labelledby="option-title" aria-describedby="option-content" >
      <Title id="option-title">分析オプション</Title>
      <Content id="option-content">
        <div class="optionfield">
          <FormField>
            <Checkbox bind:checked={kana_only} />
            <span slot="label">カナのみ分析</span>
          </FormField>
        </div>
        <div class="optionfield">
          <FormField>
            <Checkbox bind:checked={aozora} />
            <span slot="label">青空文庫モード(ルビ｜《》（）を除去)</span>
          </FormField>
        </div>
      </Content>
      <Actions>
        <Button action="accept">
          <Label>閉じる</Label>
        </Button>
      </Actions>
  </Dialog>
  <Button color="secondary" on:click={() => optionDialog.open()}><Label>オプション選択</Label></Button>

  <Dialog bind:this={remarkDialog} aria-labelledby="remark-title" aria-describedby="remark-content" >
      <Title id="remark-title">{selected_kb}</Title>
      <Content id="remark-content">
        <div class="textfield">
        {remark}
        </div>
      </Content>
      <Actions>
        <Button action="accept">
          <Label>閉じる</Label>
        </Button>
      </Actions>
  </Dialog>
  <Button color="secondary" on:click={() => {kbchange();remarkDialog.open()}}><Label>補足説明</Label></Button>

  </div>

  {#if showkb == true}

  <div style="display: flex; flex-direction: column;">

    <div class="card-container">
      <Card variant="outlined">
        <DataTable>
          <Head>
            <Row>
              <Cell>項目</Cell>
              <Cell>結果</Cell>
              <Cell></Cell>
              <Cell>項目</Cell>
              <Cell>結果</Cell>
              <Cell></Cell>
            </Row>
          </Head>
          <Body>
            <Row>
              <Cell><div class="textfield">総文字数</div></Cell>
              <Cell><div class="numberfield">{total_char}</div></Cell>
              <Cell><div class="numberfield"></div></Cell>
              <Cell><div class="textfield">総カナ文字数</div></Cell>
              <Cell><div class="numberfield">{total_kana}</div></Cell>
              <Cell><div class="numberfield"></div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">総打鍵数</div></Cell>
              <Cell><div class="numberfield">{total_key}</div></Cell>
              <Cell><div class="numberfield"></div></Cell>
              <Cell><div class="textfield">総アクション数</div></Cell>
              <Cell><div class="numberfield">{total_action}</div></Cell>
              <Cell><div class="numberfield"></div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">シフト文字数</div></Cell>
              <Cell><div class="numberfield">{total_schar}</Cell>
              <Cell><div class="numberfield">{percent(total_schar / total_kana)}</div></Cell>
              <Cell><div class="textfield">うち連続シフト数</div></Cell>
              <Cell><div class="numberfield">{renzoku_shift}</Cell>
              <Cell><div class="numberfield">{percent(renzoku_shift / total_kana)}</div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">同指連続数</div></Cell>
              <Cell><div class="numberfield">{sum(same_finger)}</div></Cell>
              <Cell><div class="numberfield">{percent(sum(same_finger) / total_action)}</div></Cell>
              <Cell><div class="textfield">うち段越え数</div></Cell>
              <Cell><div class="numberfield"></Cell>
              <Cell><div class="numberfield"></div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">左右交互打鍵数</div></Cell>
              <Cell><div class="numberfield">{total_alter}</div></Cell>
              <Cell><div class="numberfield">{percent(total_alter / total_action)}</div></Cell>
              <Cell><div class="textfield">片手連続数の平均</div></Cell>
              <Cell><div class="numberfield"></Cell>
              <Cell><div class="numberfield"></div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">アルペジオ数</div></Cell>
              <Cell><div class="numberfield">{sum(total_arpeggio)}</div></Cell>
              <Cell><div class="numberfield">{percent(sum(total_arpeggio) / total_action)}</div></Cell>
              <Cell><div class="textfield">入力できなかった文字数</div></Cell>
              <Cell><div class="numberfield">{ul}</div></Cell>
              <Cell><div class="numberfield"></div></Cell>
            </Row>
          </Body>
        </DataTable>
      </Card>
    </div>

    <div class="card-container">
      <Card style="width: 600px; margin: 3px;" variant="outlined" padded>
        キー打鍵ヒートマップ
        <div class="kbd">
          <Keyboard layout={mykeyboard} />
        </div>
      </Card>
    </div>

    <div class="card-container">
      <Card style="width: 600px; margin: 3px;" variant="outlined" padded>
        指ごとの打鍵数
        <Chart data={finger_chart} type="bar" height="200" colors ={['light-blue', 'blue', 'purple']} barOptions={{stacked:true, spaceRatio:0.5}}/>
      </Card>
    </div>

    <div class="card-container">
      <Card style="width: 600px; margin: 3px;" variant="outlined" padded>
        同じ指で連続して違うキーを打鍵した数
        <Chart data={samefinger_chart} type="bar" height="200" valuesOverPoints="1" colors ={['light-blue']} barOptions={{spaceRatio:0.5}} />
      </Card>
    </div>

    <div class="card-container">
      <Card style="width: 600px; margin: 3px;" variant="outlined" padded>
        アルペジオの詳細
        <Chart data={arpeggio_chart} type="bar" height="200" valuesOverPoints="1" colors ={['light-blue']} barOptions={{spaceRatio:0.3}} />
      </Card>
    </div>

  </div>
  {/if}
  {#if showkb == false}
  <p class="ongoing">分析を実行中...</p>
  {/if}

</main>

<style>
  main {
    padding: 1em;
    max-width: 240px;
    margin: 0 auto;
  }

  .inputfield {
    text-align: center;
    margin: 5px;
  }

  .optionfield {
    text-align: left;
  }

  .numberfield {
    text-align: right;
  }

  .textfield {
    text-align: left;
  }

  h1 {
    color: #ff3e00;
    text-transform: uppercase;
    font-size: 3em;
    font-weight: 100;
    text-align: center;
  }

  .kbd {
    display: flex;
    margin: 5px;
  }

  .ongoing {
    color: #ff3e00;
    text-align: center;
  }

  .card-container {
    display: flex;
    justify-content: center;
    align-items: center;
    min-height: 100px;
    min-width: 380px;
    margin: 3px;
  }

  @media (min-width: 640px) {
    main {
      max-width: none;
    }
  }
</style>