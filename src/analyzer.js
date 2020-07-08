  let keyboard;
  let text;
  let keyseq;
  let keydic;
  let uncounted; // 入力できなかった文字

  let finger_tandoku;
  let finger_douji;
  let finger_shifted;
  let finger_onaji;
  let arpeggio;
  
  let nkey; // 打鍵したキー数
  let nreshift; // 連続シフト
  let nkana; // 入力した文字数（かな）
  let naction;
  let nkougo; // 交互打鍵
  let nshift;
  let ndangoe;

  let last_key; // 直前に押したキー
  let shift_key;

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

  nkana = text.length;
  nkey = 0;
  nshift = 0;
  nreshift = 0;
  naction = 0;
  nkougo = 0;

  uncounted = [];
  finger_tandoku = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  finger_douji = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  finger_shifted = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  finger_onaji = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  arpeggio = new Array(keyboard.arpeggio.length);
  arpeggio.fill(0);

  shift_key = []; // 直前に押していたシフトキー
  last_key = ""; // 直前に押していたキー
  keyseq = ""; // 押したキーの羅列
}

function postprocess() {
  return {
    "nUncounted": uncounted.length,
    "uncounted": uncounted,
    "nKana": nkana,
    "nKey": nkey,
    "nAction": naction,
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
    "arpeggio": arpeggio
  };
}

function incCounter(c) {
  let mc = keyboard.conversion[c]
  // console.log(c);
  if (mc.type == "sim") {
    naction++;
  } else {
    naction += mc.keys.length + mc.shift.length;
  }
  if (mc.shift.length > 0) {
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

    if (ck in keydic && last_key in keydic) {
      // 同じ指で違うキーを連続して押す
      if (ck != last_key && keydic[ck].finger == keydic[last_key].finger) {
        finger_onaji[keydic[ck].finger]++;
      }
      // アルペジオ
      for (let i = 0; i < keyboard.arpeggio.length; i++) {
        let ar = keyboard.arpeggio[i];
        if ((ar[0] == ck && ar[1] == last_key) || (ar[1] == ck && ar[0] == last_key)) {
          arpeggio[i]++;
          // console.log(last_key, ck);
        }
      }
      // 交互打鍵
      if (((keydic[ck].finger < 5 && keydic[last_key].finger >= 5) || (keydic[ck].finger >= 5 && keydic[last_key].finger < 5))) {
        nkougo++;
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

  for (let tk of keyboard.keys.flat()) {
    tk.count = 0; // 合計
    tk.tandoku = 0; // 単独
    tk.douji = 0; // シフトではない同時押し
    tk.shifted = 0; // シフト入力
  }
  
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

  for (let k of keyboard.keys.flat()) {
    finger_tandoku[k.finger] += k.tandoku;
    finger_douji[k.finger] += k.douji;
    finger_shifted[k.finger] += k.shifted;
  }

  console.log(keyseq);
  // normalize count
  let maxv = 0;
  for (let tk of keyboard.keys.flat()) {
    if (maxv < tk.count) maxv = tk.count;
  }
  for (let tk of keyboard.keys.flat()) {
    tk.value = tk.count / maxv;
  }
  keyboard.rev = Math.random();
  // console.log(keyboard)
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
  return arr.reduce(function(prev, current, i, arr) {
    return prev+current;
  });
};
