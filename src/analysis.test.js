// import {analyzeKeyboard} from "./analyzer.js";
// import App from './App.svelte';
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

import {kanaToHira, conv_aozora, conv_kana, analyzeKeyboard, hankaku, eisuHankaku} from "./analyzer.js";

test('QWERTY', () => {
  let r = analyzeKeyboard("さっぽろでちーむにごうりゅうする。", romaji);

  expect(r.nKey).toBe(16);
  expect(r.nAction).toBe(28);
  expect(r.nTanda).toBe(28);
  expect(r.nDouji).toBe(0);
  expect(r.nShift).toBe(0);
  expect(r.nKougo).toBe(13);
  expect(r.nReShift).toBe(0);
  expect(r.nArpeggio).toBe(1);
  expect(r.nDouyubi).toBe(4);
  expect(r.nDangoe).toBe(0);
  expect(r.nHome).toBe(4);
  expect(r.nHomeNS).toBe(4);
});

test('QWERTY', () => {
  let r = analyzeKeyboard("これでじゅうぶんだとおもいますが、", romaji);

  expect(r.nKey).toBe(16);
  expect(r.nAction).toBe(27);
  expect(r.nTanda).toBe(27);
  expect(r.nDouji).toBe(0);
  expect(r.nShift).toBe(0);
  expect(r.nKougo).toBe(10);
  expect(r.nReShift).toBe(0);
  expect(r.nArpeggio).toBe(2);
  expect(r.nDouyubi).toBe(4);
  expect(r.nDangoe).toBe(0);
  expect(r.nHome).toBe(8);
  expect(r.nHomeNS).toBe(8);
});

test('新下駄', () => {
  let r = analyzeKeyboard("さっぽろでちーむにごうりゅうする。", shingeta);
  expect(r.nType).toBe(22);
  expect(r.nAction).toBe(16);
  expect(r.nShift).toBe(6);
  expect(r.nKougo).toBe(3);
});

test('新下駄', () => {
  let r = analyzeKeyboard("これでじゅうぶんだとおもいますが、", shingeta);
  expect(r.nType).toBe(21);
  expect(r.nAction).toBe(16);
  expect(r.nShift).toBe(5);
  expect(r.nKougo).toBe(4);
});

test('親指シフト', () => {
  let r = analyzeKeyboard("さっぽろでちーむにごうりゅうする。", nicola);
  expect(r.nType).toBe(28);
  expect(r.nAction).toBe(17);
  expect(r.nShift).toBe(11);
  expect(r.nKougo).toBe(5);
});

test('親指シフト', () => {
  let r = analyzeKeyboard("これでじゅうぶんだとおもいますが、", nicola);
  expect(r.nType).toBe(27);
  expect(r.nAction).toBe(17);
  expect(r.nShift).toBe(10);
  expect(r.nKougo).toBe(3);
});

test('飛鳥', () => {
  let r = analyzeKeyboard("さっぽろでちーむにごうりゅうする。", asuka);
  expect(r.nType).toBe(22);
  expect(r.nAction).toBe(17);
  expect(r.nShift).toBe(7);
  expect(r.nKougo).toBe(6);
});

test('飛鳥', () => {
  let r = analyzeKeyboard("これでじゅうぶんだとおもいますが、", asuka);
  expect(r.nType).toBe(22);
  expect(r.nAction).toBe(17);
  expect(r.nShift).toBe(10);
  expect(r.nKougo).toBe(5);
});

test('薙刀式', () => {
  let r = analyzeKeyboard("さっぽろでちーむにごうりゅうする。", naginata);

  expect(r.nKey).toBe(16);
  expect(r.nAction).toBe(16);
  expect(r.nTanda).toBe(7);
  expect(r.nDouji).toBe(4);
  expect(r.nShift).toBe(5);
  expect(r.nKougo).toBe(1);
  expect(r.nReShift).toBe(1);
  expect(r.nArpeggio).toBe(3);
  expect(r.nDouyubi).toBe(2);
  expect(r.nDangoe).toBe(0);
  expect(r.nHome).toBe(12);
  expect(r.nHomeNS).toBe(7);
});

