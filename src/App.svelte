<script>
  import Keyboard from './Keyboard.svelte';
  import Chart from 'svelte-frappe-charts';
  import kuromoji from './kuromoji/kuromoji.js';
  import {kanaToHira, conv_aozora, conv_kana, analyzeKeyboard, hankaku, eisuHankaku} from "./analyzer.js";

  // キー配列
  import romaji from './keyboards/jis_romaji.json';
  import naginata from './keyboards/jis_naginata.json';
  import ortho_naginata from './keyboards/ortho_naginata.json';
  import kana from './keyboards/jis_kana.json';
  import nicola from './keyboards/jis_nicola.json';
  import eucalyn from './keyboards/jis_eucalyn.json';
  import shingeta from './keyboards/jis_shingeta.json';
  import asuka from './keyboards/jis_asuka123.json';
  import basekit from './keyboards/ortho_basekit.json';
  import srlby from './keyboards/ortho_srlby.json';
  import colemak from './keyboards/us_colemak.json';
  import dvorak from './keyboards/us_dvorak.json';
  import qgmlwy from './keyboards/us_qgmlwy.json';
  import usbasekit from './keyboards/us_basekit.json';
  import shinjis from './keyboards/jis_shinjis.json';
  import keinarabe from './keyboards/jis_keinarabe.json';

  // Material UI
  import Textfield from '@smui/textfield';
  import Button, {Label} from '@smui/button';
  import Dialog, {Title, Content, Actions, InitialFocus} from '@smui/dialog';
  import Checkbox from '@smui/checkbox';
  import FormField from '@smui/form-field';
  import Select, {Option} from '@smui/select';
  import DataTable, {Head, Body, Row, Cell} from '@smui/data-table';
  import Card from '@smui/card';

  // 入力UI
  const keyboards = {
    "QWERTYローマ字" : romaji,
    "薙刀式": naginata,
    "薙刀式 MiniAxe": ortho_naginata,
    "JISかな": kana,
    "新JIS": shinjis,
    "親指シフト": nicola,
    "新下駄": shingeta,
    "飛鳥123": asuka,
    "けいならべ": keinarabe,
    "Eucalynローマ字": eucalyn,
    "Dvorak": dvorak,
    "Colemak": colemak,
    "QGMLWY": qgmlwy,
    "SRLBY": srlby,
    "BASEKIT OL(開発中)": basekit,
    "BASEKIT RS(開発中)": usbasekit,
  };

  let selected_kb = "QWERTYローマ字";
  let mykeyboard = keyboards[selected_kb];
  let text = "人類が増えすぎた人口を宇宙に移民させるようになって、既に半世紀が過ぎていた。地球の周りの巨大な人工都市は人類の第二の故郷となり、人々はそこで子を産み、育て、そして死んでいった。";
  let ktext = "";
  let keyseq = "";
  let showresult;
  let showkb = false;
  let kana_only = false;
  let aozora = false;
  let optionDialog;
  let remarkDialog;
  let remark = "";

  // 出力UI
  let ul; // 入力できなかった文字数
  let ntext; // 入力した文字数
  let ntype; // 打鍵したキー数
  let nkey;
  let nshift; // 打鍵したキー数、連続シフトを考慮
  let nkana; // 入力した文字数（かな）
  let ntanda;
  let ndouji;
  let nreshift;
  let naction;
  let ndouyubi;
  let ndangoe;
  let narpeggio;
  let nkougo; // 交互打鍵
  let doute;
  let nhome;
  let nhomeNS;

  // 出力グラフ
  let finger_chart;
  let samefinger_chart;
  let arpeggio_chart;
  let row_chart;

  function percent(v) {
    return (v * 100).toFixed(1) + '%'
  }

  function kbchange() {
    remark = keyboards[selected_kb].remark;
    mykeyboard = keyboards[selected_kb];
    showkb = true;
  }

  function startAnalsys() {
    mykeyboard = keyboards[selected_kb];
    showresult = false;
    let htext = eisuHankaku(text);

    kuromoji.builder({
      dicPath: 'dict' // public/dict
    }).build((error, tokenizer) => {
      // 形態素解析
      const parsed = tokenizer.tokenize(htext);
      // console.log(parsed);
      let karray = []; // 変換後のかな文字の配列
      for (let pa of parsed) {
        if (pa.pos == "記号") {
          karray.push(pa.basic_form);
        } else if (pa.reading) { // 漢字、カナ
          karray.push(kanaToHira(pa.reading));
        } else { // 英数字
          karray.push(kanaToHira(pa.surface_form));
        }
      }

      ktext = karray.join("");
      if (kana_only) {
        ktext = conv_kana(ktext);
      }
      if (aozora) {
        ktext = conv_aozora(ktext);
      }

      let r = analyzeKeyboard(ktext, mykeyboard);

      ntext = text.length;
      nkana = r.nKana;
      ntype = r.nType;
      nkey = r.nKey;
      naction = r.nAction;
      ntanda = r.nTanda;
      ndouji = r.nDouji;
      nshift = r.nShift;
      nkougo = r.nKougo;
      nreshift = r.nReShift;
      narpeggio = r.nArpeggio;
      ndouyubi = r.nDouyubi;
      ndangoe = r.nDangoe;
      doute = r.douteRenzoku;
      ul = r.nUncounted;
      nhome = r.nHome;
      nhomeNS = r.nHomeNS;
      keyseq = r.keys.join("");

      let arpeggioLegend = mykeyboard.arpeggio.map(function(a){
        return mykeyboard.keys[a[0][0]][a[0][1]].legend[0] + mykeyboard.keys[a[1][0]][a[1][1]].legend[0];
      })

      finger_chart = {
        labels: ['左小', '左薬', '左中', '左人', '左親', '右親', '右人', '右中', '右薬', '右小'],
        datasets: [
          {
            name: "単独",
            values: r.finger.tandoku
          },
          {
            name: "同時",
            values: r.finger.douji
          },
          {
            name: "シフト",
            values: r.finger.shift
          },
        ]
      };
      samefinger_chart = {
        labels: ['左小', '左薬', '左中', '左人', '左親', '右親', '右人', '右中', '右薬', '右小'],
        datasets: [
          {
            values: r.finger.onaji
          }
        ]
      };
      arpeggio_chart = {
        labels: arpeggioLegend,
        datasets: [
          {
            values: r.arpeggio
          }
        ]
      };
      row_chart = {
        labels: r.row.tandoku.map((v, i, a) => "R" + (i + 1)),
        datasets: [
          {
            name: "単独",
            values: r.row.tandoku
          },
          {
            name: "同時",
            values: r.row.douji
          },
          {
            name: "シフト",
            values: r.row.shift
          },
        ]
      };
      arpeggio_chart = {
        labels: arpeggioLegend,
        datasets: [
          {
            values: r.arpeggio
          }
        ]
      };
      showresult = true;

    });
  }
</script>

<main>
  <h1>keyboard layout analyzer</h1>

  <Textfield fullwidth textarea bind:value={text} label="入力テキスト" />

  <div class="inputfield">
    <Select bind:value={selected_kb} label="配列" >
      {#each Object.keys(keyboards) as k}
      <option>{k}</option>
      {/each}
    </Select>

  <Button color="secondary" on:click={startAnalsys} variant="outlined"><Label>分析開始</Label></Button>

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
        {#if showkb}
        <div class="kbd">
          <Keyboard layout={mykeyboard} designview=true />
        </div>
        {/if}
      </Content>
      <Actions>
        <Button action="accept" on:click={() => {showkb=false;}}>
          <Label>閉じる</Label>
        </Button>
      </Actions>
  </Dialog>
  <Button color="secondary" on:click={() => {kbchange();remarkDialog.open();}}><Label>補足説明</Label></Button>

  </div>


  {#if showresult == true}

  <div style="display: flex; flex-direction: column;">

    <div class="card-container" >
        <Textfield textarea bind:value={ktext} label="かな文字" />
    </div>
    <div class="card-container" >
        <Textfield textarea bind:value={keyseq} label="キー打鍵列" />
    </div>

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
              <Cell><div class="numberfield">{ntext}</div></Cell>
              <Cell><div class="numberfield"></div></Cell>
              <Cell><div class="textfield">総カナ文字数</div></Cell>
              <Cell><div class="numberfield">{nkana}</div></Cell>
              <Cell><div class="numberfield"></div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">総打鍵数
                  <span class="right">押したキーの数。連続シフトは1打鍵。％の母数はかな文字数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{ntype}</div></Cell>
              <Cell><div class="numberfield">{percent(ntype / nkana)}</div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">総アクション数
                  <span class="top">同時押しを1回とした数。％の母数はかな文字数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{naction}</div></Cell>
              <Cell><div class="numberfield">{percent(naction / nkana)}</div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">単打鍵数
                  <span class="right">％の母数はアクション数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{ntanda}</Cell>
              <Cell><div class="numberfield">{percent(ntanda / naction)}</div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">同時打鍵数
                  <span class="top">％の母数はアクション数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{ndouji}</Cell>
              <Cell><div class="numberfield">{percent(ndouji / naction)}</div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">使用したキー数
                  <span class="right">％の母数はアクション数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{nkey}</Cell>
              <Cell><div class="numberfield"></div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">
                  <span class="top"></span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield"></Cell>
              <Cell><div class="numberfield"></div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">ホームポジション打鍵数
                  <span class="top">％の母数は総打鍵数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{nhome}</Cell>
              <Cell><div class="numberfield">{percent(nhome / ntype)}</div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">H.P.打鍵数(除くシフト)
                  <span class="top">％の母数は総打鍵数-シフト数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{nhomeNS}</Cell>
              <Cell><div class="numberfield">{percent(nhomeNS / (ntype - nshift))}</div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">シフト文字数
                  <span class="right">シフトを押しながら入力した文字数。％の母数はアクション数。</span>
                  <i></i>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{nshift}</Cell>
              <Cell><div class="numberfield">{percent(nshift / naction)}</div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">うち連続シフト数
                  <span class="top">シフトを押っぱなしで連続して入力した文字数。％の母数はアクション数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{nreshift}</Cell>
              <Cell><div class="numberfield">{percent(nreshift / naction)}</div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">同指連続数
                  <span class="right">％の母数はアクション数-1。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{ndouyubi}</div></Cell>
              <Cell><div class="numberfield">{percent(ndouyubi / (naction - 1))}</div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">うち段越え数
                  <span class="top">同じ指で連続して行をまたいで異なるキーを押した数。％の母数はアクション数-1。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{ndangoe}</Cell>
              <Cell><div class="numberfield">{percent(ndangoe / (naction - 1))}</div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">左右交互打鍵数
                  <span class="right">親指含む。％の母数はアクション数-1。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{nkougo}</div></Cell>
              <Cell><div class="numberfield">{percent(nkougo / (naction - 1))}</div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">片手連続数の平均
                  <span class="top">親指除く。％の母数はアクション数-1。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{doute.toFixed(1)}</Cell>
              <Cell><div class="numberfield"></div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">アルペジオ数
                  <span class="right">％の母数はアクション数-1。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{narpeggio}</div></Cell>
              <Cell><div class="numberfield">{percent(narpeggio / (naction - 1))}</div></Cell>
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
        <Chart data={finger_chart} type="bar" height="230" colors ={['light-blue', 'blue', 'purple']} barOptions={{stacked:true, spaceRatio:0.5}}/>
      </Card>
    </div>

    <div class="card-container">
      <Card style="width: 600px; margin: 3px;" variant="outlined" padded>
        行ごとの打鍵数
        <Chart data={row_chart} type="bar" height="230" colors ={['light-blue', 'blue', 'purple']} barOptions={{stacked:true, spaceRatio:0.5}}/>
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
  {#if showresult == false}
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

.tooltip {
    display:inline-block;
    position:relative;
    border-bottom:1px dotted #666;
    text-align:left;
}

.tooltip .top {
    min-width: 200px;
    max-width: 600px;
    top:-20px;
    left:50%;
    transform:translate(-50%, -100%);
    padding:10px 20px;
    color:#FFFFFF;
    background-color:#444444;
    font-weight:normal;
    font-size:11px;
    border-radius:4px;
    position:absolute;
    z-index:99999999;
    box-sizing:border-box;
    display:none;
}

.tooltip:hover .top {
    display:block;
}

.tooltip .right {
    min-width:200px;
    top:50%;
    left:100%;
    margin-left:20px;
    transform:translate(0, -50%);
    padding:10px 20px;
    color:#FFFFFF;
    background-color:#444444;
    font-weight:normal;
    font-size:11px;
    border-radius:8px;
    position:absolute;
    z-index:99999999;
    box-sizing:border-box;
    display:none;
}

.tooltip:hover .right {
    display:block;
}

</style>