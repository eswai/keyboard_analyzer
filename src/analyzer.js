  let keyboard;
  let text;
  let keyseq;
  let keydic;
  let uncounted; // 入力できなかった文字
  let finger_onaji;
  let arpeggio;
  let douteList;

  let nkey; // 打鍵したキー数
  let nreshift; // 連続シフト
  let nkana; // 入力した文字数（かな）
  let naction;
  let nkougo; // 交互打鍵
  let nshift;
  let ndangoe;
  let ntandoku;
  let ndouji;

  let last_key; // 直前に押したキー
  let shift_key;
  let doute;


export function analyzeKeyboard(t, kb) {
  keyboard = kb;
  text = t;

  preprocess();
  doAnalyze();
  let result = postprocess();
  return result;
}

function preprocess() {
  // 全角英数を半角に変換
  text = text.replace(/[＂-＇＊-＞＠-ｚ]/g, function(s) {
    return String.fromCharCode(s.charCodeAt(0) - 65248);
  });

  // 文字から押したキーへの辞書を作る
  keydic = {};
  for (let mk of keyboard.keys.flat()) {
    keydic[mk.id] = mk;
  }
  for (let i = 0; i < keyboard.keys.length; i++) {
    for (let k of keyboard.keys[i]) {
      k.row = i;
    }
  }

  nkana = text.length;
  nkey = 0;
  nshift = 0;
  nreshift = 0;
  naction = 0;
  nkougo = 0;
  ndangoe = 0;
  ntandoku = 0;
  ndouji = 0;

  uncounted = [];
  finger_onaji = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

  arpeggio = new Array(keyboard.arpeggio.length);
  arpeggio.fill(0);
  douteList = [];

  shift_key = []; // 直前に押していたシフトキー
  last_key = ""; // 直前に押していたキー
  keyseq = ""; // 押したキーの羅列
  doute = 1;

  for (let tk of keyboard.keys.flat()) {
    tk.count = 0; // 合計
    tk.tandoku = 0; // 単独
    tk.douji = 0; // シフトではない同時押し
    tk.shifted = 0; // シフト入力
  }
}

function postprocess() {
  let rtandoku = [];
  let rdouji = [];
  let rshifted = [];

  let finger_tandoku = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  let finger_douji = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  let finger_shifted = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  for (let k of keyboard.keys.flat()) {
    finger_tandoku[k.finger] += k.tandoku;
    finger_douji[k.finger] += k.douji;
    finger_shifted[k.finger] += k.shifted;
  }

  for (let kr of keyboard.keys) {
    rtandoku.push(kr.reduce((acc, cur) => acc + cur.tandoku, 0));
    rdouji.push(kr.reduce((acc, cur) => acc + cur.douji, 0));
    rshifted.push(kr.reduce((acc, cur) => acc + cur.shifted, 0));
  }

  // normalize count
  let maxv = 0;
  for (let tk of keyboard.keys.flat()) {
    if (maxv < tk.count) maxv = tk.count;
  }
  for (let tk of keyboard.keys.flat()) {
    tk.value = tk.count / maxv;
  }
  keyboard.rev = Math.random();

  return {
    "nUncounted": uncounted.length,
    "uncounted": uncounted,
    "nKana": nkana,
    "nKey": nkey,
    "nAction": naction,
    "nTanda": ntandoku,
    "nDouji": ndouji,
    "nShift": nshift,
    "nReShift": nreshift,
    "nDouyubi": sum(finger_onaji),
    "nDangoe": ndangoe,
    "nKougo": nkougo,
    "nArpeggio": sum(arpeggio),
    "keyboard": keyboard,
    "finger": {
      "tandoku": finger_tandoku,
      "douji": finger_douji,
      "shift": finger_shifted,
      "onaji": finger_onaji
    },
    "row": {
      "tandoku": rtandoku,
      "douji": rdouji,
      "shift": rshifted,
    },
    "arpeggio": arpeggio,
    "douteRenzoku": sum(douteList) / douteList.length
  };
}

function incCounter(c) {
  let mc = keyboard.conversion[c]
  // console.log(c);
  if (mc.type == "sim") { // 同時打鍵
    naction++;
    if (mc.keys.length + mc.shift.length == 1) {
      ntandoku++;
    } else {
      ndouji++;
    }
  } else { // 連続打鍵
    naction += mc.keys.length + mc.shift.length;
    ntandoku += mc.keys.length + mc.shift.length;
  }

  if (mc.shift.length > 0) { // シフトあり
    nshift++;
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
    nkey++;
    keyseq += ck;

    // 連続打鍵、または同時打鍵で単打のとき（シフトでない同時打鍵は除く）
    if (mc.type == 'seq' || (mc.type == 'sim' && mc.keys.length == 1)) {
      if (ck in keydic && last_key in keydic) {
        // 同じ指で違うキーを連続して押す
        if (ck != last_key && keydic[ck].finger == keydic[last_key].finger) {
          finger_onaji[keydic[ck].finger]++;
          if (keydic[last_key].row != keydic[ck].row) {
            ndangoe++;
          }
        }
        // アルペジオ
        for (let i = 0; i < keyboard.arpeggio.length; i++) {
          let ar = keyboard.arpeggio[i];
          let k1 = keyboard.keys[ar[0][0]][ar[0][1]];
          let k2 = keyboard.keys[ar[1][0]][ar[1][1]];
          if ((k1.id == ck && k2.id == last_key) || (k2.id == ck && k1.id == last_key)) {
            arpeggio[i]++;
            // console.log(last_key, ck);
          }
        }
        // 交互打鍵
        if (((keydic[ck].finger <= 3 && keydic[last_key].finger >= 6) || (keydic[ck].finger >= 6 && keydic[last_key].finger <= 3))) {
          nkougo++;
          if (doute > 1) douteList.push(doute);
          doute = 1;
        } else {
          doute++;
        }
      }
    }
    last_key = ck;
  }

  if (mc.shift.length == 0) { // シフトキーを押していない
    shift_key = [];
  } else { // シフトキーを押している
    for (let cs of mc.shift) {
      // 連続シフト
      if (mc.renzsft && shift_key.includes(cs)) {
        nreshift++;
      } else {
        keydic[cs].count++;
        nkey++;
        keydic[cs].shifted++;
      }
    }
    shift_key = mc.shift;
  }

}

function doAnalyze() {
  console.log(text);

  for (let i = 0; i < text.length; i++) {
    // console.log(text.charAt(i));
    let ch1 = text.charAt(i);
    let ch2 = text.substr(i, 2);
    let ch3 = text.substr(i, 3);
    if (ch3 in keyboard.conversion) {
      incCounter(ch3);
      i += 2;
    } else if (ch2 in keyboard.conversion) {
      incCounter(ch2);
      i++;
    } else if (ch1 in keyboard.conversion) {
      incCounter(ch1)
    } else {
      uncounted.push(ch1);
    }
  }
  if (doute > 1) douteList.push(doute);

  console.log(keyseq);
  console.log(uncounted);

  // console.log(finger_count);

}

export function kanaToHira(str) {
  return str.replace(/[\u30a1-\u30f6]/g, function(match) {
    let chr = match.charCodeAt(0) - 0x60;
    return String.fromCharCode(chr);
  });
}

export function conv_aozora(text) {
  t = text.replace(/（.+?）/g, "");
  t = t.replace(/《.+?》/g, "");
  t = t.replace(/｜/g, "");
  return t;
}

export function conv_kana(text) {
  return text.replace(/[\u0000-\u2999\u3003-\u3040\u3097-\uff00\uff02-\uff1e\uff20-\uffff]/g, "");
}

function sum(arr) {
  if (arr.length == 0) {
    return 0;
  } else {
    return arr.reduce(function(prev, current, i, arr) {
      return prev+current;
    });
  }

};
