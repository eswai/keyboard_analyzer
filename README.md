# 日本語キー配列アナライザー

## 目的

すでに英文を対象にしたキーボードのキー配列分析ソフトとしてはいくつかあって、例えば[Keyboard Layout Analyzer](http://patorjk.com/keyboard-layout-analyzer/#/main)や[QMK Keymap Carpalx Analyser](https://jackhumbert.github.io/typing_model/)があります。
しかし、これらは英文を対象としており、新下駄や薙刀式のようにローマ字ではないカナ入力方式を分析しようとすると、かなり手間がかかることになります。
そこで、英文同様の操作で日本語の文章をキーボードで入力する労力を分析し、またローマ字入力だけではなく、より先進的なカナ入力方式を分析して、よりよい入力方式を創造していく一助になるべく、日本語キー配列アナライザーを作って行くのが、このプロジェクトの目的です。

## 詳細

すでにいくつかの英字配列、カナ配列をいくつかjson形式で定義しています。

`keys`はキーボードの配列を２次元配列で定義しています。sizeはキーの幅で単位はUです。
`id`はそのキーを一意に表します。
`legend`は配列表に表示する記号です。3文字まで対応します。空配列にするとキーの枠も表示しないので、分割式キーボードのように、表示上のスペースを作れます。
`fingerは`そのキーを何指で押すかを示しており、左小指から右小指に向かって順に0, 1, 2と番号をふっています。
どの指でキーを押すかは個人差があるところですので、定義ファイルを自分に合わせて書き換えてください。
OrtholinearやUSキーボードなどの定義をすることができますが、column staggeredのように縦にずれた配列は表現できません。

`arpeggio`は2キー連続打鍵で快適にうてる組み合わせを定義します。
keysの行列位置を`[[行, 列], [行, 列]]`で定義しています。
キーのidで指定していないのは、英字の配列を変更してもアルペジオ定義を変更しなくてもいいようにするためです。

`conversion`は文字を入力するのにどのように入力するかを定義します。単にキーをおすだけか、シフトキーを押しながら、ローマ字のように2キーを連続して押すのか、を定義します。
`keys`に定義しても`shift`に定義しても押下頻度の計算は同じですが、追って連続シフト率などを計算したいので、別にしています。
`type`はローマ字のように連続して打鍵する`seq`のか、親指シフトのように同時に打鍵する`sim`のかを定義します。
連続シフトが可能な場合は`renzsft: true`とします。

## 謝辞

漢字まじりの文章をカナ文字に変換するために[kuromoji.js](https://github.com/takuyaa/kuromoji.js)を使用させていただいています。

## テストサイト

https://keyboard-analyzer.vercel.app/

![](screenshot.png)