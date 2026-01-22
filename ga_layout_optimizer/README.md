# GA Layout Optimizer

遺伝的アルゴリズムによる日本語キーボード配列最適化ツール。

## 特徴

- **10種類の評価軸**による多目的最適化
- **GPU対応**（WGPU/WebGPU）で大規模並列計算
- **再現可能**なシード値による実行
- 既存のkeyboard_analyzer形式のJSON出力

## 評価軸と重み

| 評価軸 | 重み | 説明 |
|--------|------|------|
| 1段飛ばし少なさ | 7 | 同じ指で2段以上移動する頻度を減らす |
| ホームポジション率 | 6 | ホームポジションキー使用率 |
| 総打鍵数少なさ | 5 | 位置重み付き総打鍵コスト |
| 同指連続率低さ | 4 | 同じ指で連続して打つ頻度を減らす |
| 単打鍵数多さ | 3 | シフトなしで入力できる文字数 |
| シフト文字少なさ | 2 | シフトが必要な文字の頻度 |
| Colemak類似性 | 2 | 英語Colemakとの母音・子音位置の類似度 |
| 月配列類似性 | 1 | 月配列2-263との類似度 |
| 覚えやすさ | 1 | シフト面での母音位置一致 |
| 左右交互打鍵率 | 1 | 左右の手を交互に使う頻度 |

## インストール

```bash
cd ga_layout_optimizer
cargo build --release
```

## 使い方

### 基本使用

```bash
./target/release/ga_layout_optimizer --corpus ../sample.txt
```

### オプション

```
-c, --corpus <FILE>        評価用テキストファイル [default: ../sample.txt]
-p, --population <N>       集団サイズ [default: 500]
-g, --generations <N>      世代数 [default: 1000]
-m, --mutation-rate <F>    突然変異率 (0.0-1.0) [default: 0.15]
-e, --elite <N>            エリート個体数 [default: 10]
-s, --seed <N>             ランダムシード (再現性確保) [default: 42]
-o, --output <FILE>        出力ファイル名 [default: best_layout.json]
    --gpu                  GPU加速を有効化
-t, --threads <N>          スレッド数 (0=自動) [default: 0]
```

### 実行例

```bash
# CPU高速最適化（推奨）
./target/release/ga_layout_optimizer \
    --population 500 \
    --generations 2000 \
    --seed 12345 \
    --output my_layout.json

# GPU加速（対応GPUが必要）
./target/release/ga_layout_optimizer \
    --gpu \
    --population 1000 \
    --generations 5000
```

## 出力形式

### best_layout.json

最適化結果の詳細情報を含むJSON:

```json
{
  "name": "GA Optimized Layout (seed: 42)",
  "fitness": 2243.03,
  "generation": 500,
  "scores": {
    "row_skip": 99.51,
    "home_position": 61.47,
    ...
  },
  "layers": {
    "no_shift": [["に", "み", ...], ...],
    "a_shift": [...],
    "b_shift": [...]
  }
}
```

### ga_layout_seed*_*.json

keyboard_analyzer互換形式のJSON。Webアプリで直接読み込み可能。

## 配列構造

3レイヤー構造:
- **No Shift**: シフトなし（最頻出文字用）
- **A Shift**: 中指シフトA（中頻度文字用）
- **B Shift**: 中指シフトB（低頻度文字用）

キー配置（3行×10列）:
```
Q  W  E  R  T    Y  U  I  O  P
A  S  D  F  G    H  J  K  L  ;
Z  X  C  V  B    N  M  ,  .  /
```

## アルゴリズム

1. **初期化**: ランダム + スマート初期化（頻出文字をホームポジションに配置）
2. **評価**: 10種類の指標で適合度を計算
3. **選択**: トーナメント選択（k=3）
4. **交配**: 一様交叉（レイヤー単位）
5. **突然変異**: ランダムスワップ
6. **修復**: 重複文字の除去

## 依存関係

- Rust 1.70+
- wgpu（GPU加速用、オプション）
- rayon（CPU並列処理）

## ライセンス

MIT License

## 参考

- [scala-ga-layout](https://github.com/Harsiharsi/scala-ga-layout) - 参考にしたScala実装
- [keyboard_analyzer](../README.md) - 評価用Webアプリ
