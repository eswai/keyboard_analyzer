  let keyboard;
  let text;
  let keyseq;
  let keydic;
  let uncounted; // 入力できなかった文字
  let finger_onaji;
  let arpeggio;
  let douteList;

  let ntype; // 打鍵したキー数
  let nkey;
  let nreshift; // 連続シフト
  let nkana; // 入力した文字数（かな）
  let naction;
  let nkougo; // 交互打鍵
  let nshift;
  let ndangoe;
  let ntandoku;
  let ndouji;
  let nhome;
  let nhomeNS; // シフトを除くホームポジション打件数

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

export function hankaku(s) {
  // 全角を半角に変換
  let hs = s.replace(/[！-～]/g,
    function(v) {
      return String.fromCharCode(v.charCodeAt(0) - 0xFEE0);
    }
  );

  return hs.replace(/”/g, "\"")
    .replace(/’/g, "'")
    .replace(/‘/g, "`")
    .replace(/￥/g, "\\")
    .replace(/　/g, " ")
    .replace(/〜/g, "~");
}

export function eisuHankaku(s) {
  // 全角英数を半角に変換
  let hs = s.replace(/[Ａ-Ｚａ-ｚ０-９]/g,
    function(v) {
      return String.fromCharCode(v.charCodeAt(0) - 0xFEE0);
    }
  );
  return hs;
}

function preprocess() {
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
  ntype = 0;
  nkey = new Set();
  nshift = 0;
  nreshift = 0;
  naction = 0;
  nkougo = 0;
  ndangoe = 0;
  ntandoku = 0;
  ndouji = 0;
  nhome = 0;
  nhomeNS = 0;

  uncounted = [];
  finger_onaji = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

  arpeggio = new Array(keyboard.arpeggio.length);
  arpeggio.fill(0);
  douteList = [];

  shift_key = []; // 直前に押していたシフトキー
  last_key = ""; // 直前に押していたキー
  keyseq = []; // 押したキーの羅列
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

  // 左手
  for (let kr of keyboard.keys) {
    rtandoku.push(kr.reduce((acc, cur) => cur.finger <= 4 ? acc + cur.tandoku : acc, 0));
    rdouji.push(kr.reduce((acc, cur) => cur.finger <= 4 ? acc + cur.douji : acc, 0));
    rshifted.push(kr.reduce((acc, cur) => cur.finger <= 4 ? acc + cur.shifted : acc, 0));
  }
  rtandoku.reverse();
  rdouji.reverse();
  rshifted.reverse();
  // 右手
  for (let kr of keyboard.keys) {
    rtandoku.push(kr.reduce((acc, cur) => cur.finger >= 5 ? acc + cur.tandoku : acc, 0));
    rdouji.push(kr.reduce((acc, cur) => cur.finger >= 5 ? acc + cur.douji : acc, 0));
    rshifted.push(kr.reduce((acc, cur) => cur.finger >= 5 ? acc + cur.shifted : acc, 0));
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
    "nType": ntype,
    "nKey": nkey.size,
    "nAction": naction,
    "nTanda": ntandoku,
    "nDouji": ndouji,
    "nShift": nshift,
    "nReShift": nreshift,
    "nDouyubi": sum(finger_onaji),
    "nDangoe": ndangoe,
    "nKougo": nkougo,
    "nArpeggio": sum(arpeggio),
    "nHome": nhome,
    "nHomeNS": nhomeNS,
    "keyboard": keyboard,
    "finger": {
      "tandoku": finger_tandoku,
      "douji": finger_douji,
      "shift": finger_shifted,
      "onaji": finger_onaji
    },
    "left": sum(finger_tandoku.slice(0, 5)) + sum(finger_shifted.slice(0, 5)) + sum(finger_douji.slice(0, 5)),
    "right": sum(finger_tandoku.slice(5, 10)) + sum(finger_shifted.slice(5, 10)) + sum(finger_douji.slice(5, 10)),
    "row": {
      "tandoku": rtandoku,
      "douji": rdouji,
      "shift": rshifted,
    },
    "arpeggio": arpeggio,
    "douteRenzoku": sum(douteList) / douteList.length,
    "keys": keyseq.map((x) => x.shift.map((y) => "<" + y + ">").concat(x.keys)).flat(),
  };
}

function evaluateKeyCombination(c1, c0) {
  let c1ks = c1.keys.concat(c1.shift); // シフトも含めたキー列
  let c0ks = c0.keys.concat(c0.shift);

  if (c1.type == "sim") { // 同時打鍵
    naction++;
    if (c1.shift.length > 0) {
      nshift++;
    } else if (c1.keys.length == 1) { // 単独押し
      ntandoku++;
    } else {
      ndouji++;
    }
  } else { // 順次打鍵seq
    naction += c1.keys.length;
    if (c1.shift.length > 0) {
      nshift++;
    } else if (c1.keys.length == 1) { // 単独押し
      ntandoku++;
    }
  }

  for (let ck of c1.keys) {
    if (c1.shift.length > 0) {
      keydic[ck].shifted++;
    } else if (c1.type == 'sim' && c1.keys.length > 1) {
      keydic[ck].douji++;
    } else {
      keydic[ck].tandoku++;
    }
    keydic[ck].count++;
    ntype++;
  }

  if (c1.shift.length == 0) { // シフトキーを押していない
    shift_key = [];
  } else { // シフトキーを押している
    for (let cs of c1.shift) {
      // 連続シフト
      if (c1.renzsft && shift_key.includes(cs)) {
        nreshift++;
      } else {
        keydic[cs].count++;
        ntype++;
        keydic[cs].shifted++;
      }
    }
    shift_key = c1.shift;
  }

  // 同じ指で違うキーを連続して押す
  // let c1f = c1.keys.map((a) => keydic[a].finger);
  // let c0f = c0.keys.map((a) => keydic[a].finger);

  // 同じキーを除外する
  let c10un = c1ks.concat(c0ks).filter(function(x, i, self) { // 重複削除
    return self.indexOf(x) === i;
  });
  let c10unf = c10un.map((a) => keydic[a].finger);

  let c10douy = c10unf.filter(function(x, i, self) { // 重複を抜き出す
    return self.indexOf(x) !== i;
  });
  c10douy.map(x => finger_onaji[x]++);

  // アルペジオ
  // 1アクションの中のアルペジオ
  // 薙刀式　じょ
  findArpeggio(c1ks);

  // 連続シフトできるとき
  // シフトキーを除いてはいけない、シフトキーが変わるケースがある
  // 連続シフトなので同じシフトキー押しっぱなしはいい
  let c1fs = c1ks.map((a) => keydic[a].finger);
  let c0fs = c0ks.map((a) => keydic[a].finger);

  let c0un = c0fs.filter(function(x, i, self) { // 重複削除
    return self.indexOf(x) === i;
  });
  let c10fs = c0un.concat(c1fs);

  if (c1.renzsft) {
    // 薙刀式　あい、ある、がる、のる
    if (duplicated(c10fs) == 0) { // 同じ指が含まれていない
      for (let k of c0ks) { // 2アクション間のアルペジオ
        let k10 = c1ks.concat(k);
        findArpeggio(k10);
      }
    } else {
      // でも同じ指が同じシフトキーならok
      // 薙刀式　もの
      // 薙刀式　がで、はアルペジオだと思うが、濁点はシフトではなく同時押しなので、ここではカウントしない
      // シフトキー以外が同じ指打鍵のケースを排除できていない
      if (c1.shift.sort().join() == c0.shift.sort().join()) {
        for (let k of c0ks) { // 2アクション間のアルペジオ
          let k10 = c1ks.concat(k);
          findArpeggio(k10);
        }
      }
    }
  // 連続シフトできないとき
  // 同じシフトキーであっても押し直すので、連続したらアルペジオではない
  } else {
    if (duplicated(c10fs) == 0) { // 同じ指が含まれていない
      for (let k of c0ks) { // 2アクション間のアルペジオ
        let k10 = c1ks.concat(k);
        findArpeggio(k10);
      }
    }
  }

  // 交互打鍵、同手連続
  let h1 = hand(c1ks);
  let h0 = hand(c0ks);
  if ((h1 == "left" && h0 == "left") || (h1 == "right" && h0 == "right")) {
    doute++;
  } else if ((h1 == "left" && h0 == "right") || (h1 == "right" && h0 == "left")) {
    nkougo++;
    if (doute > 1) douteList.push(doute);
    doute = 1;
  }

}

function doAnalyze() {
  console.log(text);

  // キー打鍵列へ変換する
  for (let i = 0; i < text.length; i++) {
    // console.log(text.charAt(i));
    let fuc = true;
    for (let j = 3; j > 0; j--) {
      let ch = text.substr(i, j);
      if (ch in keyboard.conversion) {
        fuc = false;
        let kc = keyboard.conversion[ch];
        if (kc.type == "seq") {
          for (let k of kc.keys) {
            let a = Object.assign({}, kc);
            a.keys = [k];
            keyseq.push(a);
          }
        } else {
          keyseq.push(keyboard.conversion[ch]);
        }
        i += j - 1;
        break;
      }
    }
    if (fuc) {
      for (let j = 3; j > 0; j--) {
        let ch = text.substr(i, j);
        ch = hankaku(ch);
        if (ch in keyboard.conversion) {
          fuc = false;
          let kc = keyboard.conversion[ch];
          if (kc.type == "seq") {
            for (let k of kc.keys) {
              let a = Object.assign({}, kc);
              a.keys = [k];
              keyseq.push(a);
            }
          } else {
            keyseq.push(keyboard.conversion[ch]);
          }
          i += j - 1;
          break;
        }
      }
    }

    if (fuc) {
      uncounted.push(text.substr(i, 1));
    }
  }

  console.log(keyseq);

  let prev = {"keys": [], "shift":[], "type": "seq", "ime": false};
  for (let i = 0; i < keyseq.length; i++) {
    evaluateKeyCombination(keyseq[i], prev);
    prev = keyseq[i];
    keyseq[i].keys.concat(keyseq[i].shift).map(function(x) {
      nkey.add(x);
      if (keydic[x].home) {
        nhome++;
      }
    });
    keyseq[i].keys.map(function(x) {
      if (keydic[x].home) {
        nhomeNS++;
      }
    });
  }
  if (doute > 1) douteList.push(doute);

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

function hand(keys) {
  if (keys.length == 0) {
    return false;
  } else if (keys.every((val, i, arr) => keydic[val].finger <= 4)) {
    return "left";
  } else if (keys.every((val, i, arr) => keydic[val].finger >= 5)) {
    return "right";
  } else {
    return "both";
  }
}

function findArpeggio(keys) {
  for (let i = 0; i < keyboard.arpeggio.length; i++) {
    let ar = keyboard.arpeggio[i];
    let k1 = keyboard.keys[ar[0][0]][ar[0][1]];
    let k2 = keyboard.keys[ar[1][0]][ar[1][1]];
    if (keys.includes(k1.id) && keys.includes(k2.id)) {
      arpeggio[i]++;
    }
  }
}

function duplicated(a){
  var s = new Set(a);
  return a.length - s.size;
}

export function numKanji(t){
  if (t) {
    let m = t.match(/[々〇〻\u3400-\u9FFF\uF900-\uFAFF]|[\uD840-\uD87F][\uDC00-\uDFFF]/g);
    if (m) {
      return m.reduce((acc, val) => acc + val.length, 0);
    }
  }
  return 0;
}

export function numEisu(t){
  if (t) {
    let m = t.match(/[Ａ-Ｚａ-ｚ０-９！”＃＄％＆’（）＝～｜‘｛＋＊｝＜＞？＿－＾￥＠「；：」、。・!-~]/g);
    if (m) {
      return m.reduce((acc, val) => acc + val.length, 0);
    }
  }
  return 0;
}