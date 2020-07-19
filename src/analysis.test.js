// import {analyzeKeyboard} from "./analyzer.js";
// import App from './App.svelte';
import romaji from './keyboards/jis_romaji.json';
import {kanaToHira, conv_aozora, conv_kana, analyzeKeyboard, hankaku, eisuHankaku} from "./analyzer.js";

test('QWERTY', () => {
  let r = analyzeKeyboard("さっぽろでちーむにごうりゅうする。", romaji);
  expect(r.nAction).toBe(28);
  expect(r.nShift).toBe(0);
});

