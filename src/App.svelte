<script>
  import {onMount} from 'svelte';
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
  import colevrak from './keyboards/ortho_colevrak.json';
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
    "Colevrak(開発中)": colevrak,
    "BASEKIT OL(開発中)": basekit,
    "BASEKIT RS(開発中)": usbasekit,
  };

  let tokenizer;

  let selected_kb = "QWERTYローマ字";
  let mykeyboard = keyboards[selected_kb];
  let text = "人類が増えすぎた人口を宇宙に移民させるようになって、既に半世紀が過ぎていた。地球の周りの巨大な人工都市は人類の第二の故郷となり、人々はそこで子を産み、育て、そして死んでいった。";
  let ktext = "";
  let keyseq = "";
  let showresult = "loading";
  let kana_only = false;
  let skip_conv = false;
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
  let left;
  let right;

  // 出力グラフ
  let finger_chart;
  let samefinger_chart;
  let arpeggio_chart;
  let row_chart;

  // 辞書をロード
  onMount(() => {
    kuromoji.builder(
        {dicPath: 'dict'} // public/dict
      ).build((error, _tokenizer) => {
        tokenizer = _tokenizer;
        showresult = "ready";
        }
      );
    }
  );

  function percent(v, n) {
    return (v * 100).toFixed(n) + '%'
  }

  function kbchange() {
    remark = keyboards[selected_kb].remark;
    mykeyboard = keyboards[selected_kb];
  }

  function startAnalsys() {
    showresult = "running";

    mykeyboard = keyboards[selected_kb];
    let htext = eisuHankaku(text);

    // 形態素解析
    if (skip_conv) {
      ktext = htext;
    } else {
      let parsed = tokenizer.tokenize(htext);

      // console.log(parsed);
      let karray = []; // 変換後のかな文字の配列
      for (let pa of parsed) {
        if (pa.pos == "記号") {
          karray.push(pa.surface_form);
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
    left = r.left;
    right = r.right;
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
      labels: r.row.tandoku.map((v, i, a) => i < r.row.tandoku.length / 2 ? "左R" + (r.row.tandoku.length / 2 - i) : "右R" + (i - r.row.tandoku.length / 2 + 1)),
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
    showresult = "finished";
  }
</script>


<main>

  <a href="https://github.com/eswai/keyboard_analyzer" class="github-corner" aria-label="View source on GitHub">
  <svg width="80" height="80" viewBox="0 0 250 250" style="fill:#151513; color:#fff; position: absolute; top: 0; border: 0; right: 0;" aria-hidden="true"><path d="M0,0 L115,115 L130,115 L142,142 L250,250 L250,0 Z"></path><path d="M128.3,109.0 C113.8,99.7 119.0,89.6 119.0,89.6 C122.0,82.7 120.5,78.6 120.5,78.6 C119.2,72.0 123.4,76.3 123.4,76.3 C127.3,80.9 125.5,87.3 125.5,87.3 C122.9,97.6 130.6,101.9 134.4,103.2" fill="currentColor" style="transform-origin: 130px 106px;" class="octo-arm"></path><path d="M115.0,115.0 C114.9,115.1 118.7,116.5 119.8,115.4 L133.7,101.6 C136.9,99.2 139.9,98.4 142.2,98.6 C133.8,88.0 127.5,74.4 143.8,58.0 C148.5,53.4 154.0,51.2 159.7,51.0 C160.3,49.4 163.2,43.6 171.4,40.1 C171.4,40.1 176.1,42.5 178.8,56.2 C183.1,58.6 187.2,61.8 190.9,65.4 C194.5,69.0 197.7,73.2 200.1,77.6 C213.8,80.2 216.3,84.9 216.3,84.9 C212.7,93.1 206.9,96.0 205.4,96.6 C205.1,102.4 203.0,107.8 198.3,112.5 C181.9,128.9 168.3,122.5 157.7,114.1 C157.9,116.9 156.7,120.9 152.7,124.9 L141.0,136.5 C139.8,137.7 141.6,141.9 141.8,141.8 Z" fill="currentColor" class="octo-body"></path></svg></a><style>.github-corner:hover .octo-arm{animation:octocat-wave 560ms ease-in-out}@keyframes octocat-wave{0%,100%{transform:rotate(0)}20%,60%{transform:rotate(-25deg)}40%,80%{transform:rotate(10deg)}}@media (max-width:500px){.github-corner:hover .octo-arm{animation:none}.github-corner .octo-arm{animation:octocat-wave 560ms ease-in-out}}</style>

  <h1>keyboard layout analyzer</h1>

  <Textfield fullwidth textarea bind:value={text} label="入力テキスト" />

  <div class="inputfield">
    <Select bind:value={selected_kb} label="配列" >
      {#each Object.keys(keyboards) as k}
      <option>{k}</option>
      {/each}
    </Select>

  {#if showresult == "loading"}
  <Button color="secondary" on:click={startAnalsys} variant="outlined" disabled><Label>分析開始</Label></Button>
  {:else}
  <Button color="secondary" on:click={startAnalsys} variant="outlined"><Label>分析開始</Label></Button>
  {/if}

  <Dialog bind:this={optionDialog} aria-labelledby="option-title" aria-describedby="option-content" >
      <Title id="option-title">分析オプション</Title>
      <Content id="option-content">
        <div class="optionfield">
          <FormField>
            <Checkbox bind:checked={skip_conv} />
            <span slot="label">漢字をカナ変換しない (形態素解析をバイパス)</span>
          </FormField>
        </div>
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
        <div class="kbd">
          <Keyboard layout={mykeyboard} designview=true />
        </div>
      </Content>
      <Actions>
        <Button action="accept">
          <Label>閉じる</Label>
        </Button>
      </Actions>
  </Dialog>
  <Button color="secondary" on:click={() => {kbchange();remarkDialog.open();}}><Label>補足説明</Label></Button>

  </div>


  {#if showresult == "finished"}

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
              <Cell><div class="numberfield">{percent(ntype / nkana, 1)}</div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">総アクション数
                  <span class="top">同時押しを1回とした数。％の母数はかな文字数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{naction}</div></Cell>
              <Cell><div class="numberfield">{percent(naction / nkana, 1)}</div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">単打鍵数
                  <span class="right">％の母数はアクション数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{ntanda}</Cell>
              <Cell><div class="numberfield">{percent(ntanda / naction, 1)}</div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">同時打鍵数
                  <span class="top">％の母数はアクション数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{ndouji}</Cell>
              <Cell><div class="numberfield">{percent(ndouji / naction, 1)}</div></Cell>
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
                左手:右手
              </div></Cell>
              <Cell><div class="numberfield">{left}:{right}</Cell>
              <Cell><div class="numberfield">{percent(left/ntype, 0)}:{percent(right/ntype, 0)}</div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">ホームポジション打鍵数
                  <span class="right">％の母数は総打鍵数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{nhome}</Cell>
              <Cell><div class="numberfield">{percent(nhome / ntype, 1)}</div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">H.P.打鍵数(除くシフト)
                  <span class="top">％の母数は総打鍵数-シフト数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{nhomeNS}</Cell>
              <Cell><div class="numberfield">{percent(nhomeNS / (ntype - nshift), 1)}</div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">シフト文字数
                  <span class="right">シフトを押しながら入力した文字数。％の母数はアクション数。</span>
                  <i></i>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{nshift}</Cell>
              <Cell><div class="numberfield">{percent(nshift / naction, 1)}</div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">うち連続シフト数
                  <span class="top">シフトを押っぱなしで連続して入力した文字数。％の母数はアクション数。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{nreshift}</Cell>
              <Cell><div class="numberfield">{percent(nreshift / naction, 1)}</div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">同指連続数
                  <span class="right">％の母数はアクション数-1。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{ndouyubi}</div></Cell>
              <Cell><div class="numberfield">{percent(ndouyubi / (naction - 1), 1)}</div></Cell>
              <Cell><div class="textfield">
                <div class="tooltip">うち段越え数
                  <span class="top">同じ指で連続して行をまたいで異なるキーを押した数。％の母数はアクション数-1。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{ndangoe}</Cell>
              <Cell><div class="numberfield">{percent(ndangoe / (naction - 1), 1)}</div></Cell>
            </Row>
            <Row>
              <Cell><div class="textfield">
                <div class="tooltip">左右交互打鍵数
                  <span class="right">親指含む。％の母数はアクション数-1。</span>
                </div>
              </div></Cell>
              <Cell><div class="numberfield">{nkougo}</div></Cell>
              <Cell><div class="numberfield">{percent(nkougo / (naction - 1), 1)}</div></Cell>
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
              <Cell><div class="numberfield">{percent(narpeggio / (naction - 1), 1)}</div></Cell>
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
        行ごとの打鍵数 (R1が最上段)
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

  {#if showresult == "loading"}
  <p class="ongoing">辞書をロード中</p>
  {/if}

  {#if showresult == "running"}
  <p class="ongoing">解析中</p>
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