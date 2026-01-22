# GA Keyboard Layout Optimizer

日本語かな配列を遺伝的アルゴリズム(GA)で最適化するRust実装。GPU加速対応。

## 実行方法

### 単一実行

```bash
# n-gramファイルを使用（推奨・高精度）
./target/release/ga_layout_optimizer \
  --gram1 ../1gram.txt --gram2 ../2gram.txt \
  --gram3 ../3gram.txt --gram4 ../4gram.txt \
  -p 500 -g 2000 -s 42

# コーパステキストを使用（フォールバック）
./target/release/ga_layout_optimizer -c ../corpus_1m.txt -p 500 -g 1000
```

### マルチラン実行（推奨・統計的信頼性向上）

```bash
# CPUコア数を最大活用（32コアなら32並列実行）
./target/release/ga_layout_optimizer \
  --gram1 ../1gram.txt --gram2 ../2gram.txt \
  --gram3 ../3gram.txt --gram4 ../4gram.txt \
  -p 500 -g 1000 \
  --multi-run 32

# CPUコア数を超える指定も自動調整
./target/release/ga_layout_optimizer \
  --gram1 ../1gram.txt --gram2 ../2gram.txt \
  --gram3 ../3gram.txt --gram4 ../4gram.txt \
  -p 500 -g 1000 \
  --multi-run 100  # 自動的にCPUコア数まで制限
```

**マルチラン機能:**
- 完全ランダムなシード値で複数のGA実行を並列化
- CPUコア数を自動検出し、上限まで並列実行
- 指定数がCPUコア数を超える場合は自動調整
- 全結果の統計分析（平均、標準偏差、最良/最悪）
- 最良配列を自動選択して保存
- TUIは無効化され、コンソール出力のみ
- `-s/--seed`オプションは無視（完全ランダム）

### 主要オプション

| オプション | デフォルト | 説明 |
|-----------|----------|------|
| `--gram1` | - | 1-gramファイル |
| `--gram2` | - | 2-gramファイル |
| `--gram3` | - | 3-gramファイル |
| `--gram4` | - | 4-gramファイル |
| `-c, --corpus` | `../sample.txt` | コーパスファイル (n-gram未指定時) |
| `-p, --population` | 500 | 集団サイズ |
| `-g, --generations` | 1000 | 世代数 |
| `-m, --mutation-rate` | 0.15 | 突然変異率 |
| `-e, --elite` | 10 | エリート保持数 |
| `-s, --seed` | 42 | 乱数シード（再現性） |
| `--no-gpu` | false | GPU無効化 |
| `--tui` | false | TUI可視化モード（単一実行のみ） |
| `--multi-run` | 0 | 並列実行数（0=単一実行） |

### 評価重みオプション（カスタマイズ）

**Core Metrics (乗算・指数):**
- `--w-same-finger` (1.8): 同指連続率の低さ
- `--w-row-skip` (1.55): 段越えの少なさ
- `--w-home-position` (1.3): ホームポジション率
- `--w-total-keystrokes` (1.05): 総打鍵コスト
- `--w-alternating` (0.8): 左右交互打鍵率
- `--w-single-key` (0.7): 単打鍵率
- `--w-colemak-similarity` (0.6): Colemak類似度

**Bonus Metrics (加算・重み):**
- `--w-redirect-low` (5.0): リダイレクト少
- `--w-tsuki-similarity` (4.0): 月配列類似度
- `--w-roll` (5.0): ロール率
- `--w-inroll` (5.0): インロール率
- `--w-arpeggio` (5.0): アルペジオ率
- `--w-memorability` (2.0): 覚えやすさ
- `--w-shift-balance` (3.0): シフトバランス（☆★均等化）

**例: 同指連続回避を最優先:**
```bash
./target/release/ga_layout_optimizer \
  --gram1 ../1gram.txt --gram2 ../2gram.txt \
  --gram3 ../3gram.txt --gram4 ../4gram.txt \
  -p 500 -g 1000 \
  --w-same-finger 2.5 \
  --w-row-skip 2.0
```

## 配列構造

3層構造のかな配列（前置シフト対応）：

- **Layer 0（無シフト）**: 高頻度文字（1打鍵）
- **Layer 1（☆シフト）**: シフト+文字（2打鍵・dキー前置）
- **Layer 2（★シフト）**: シフト+文字（2打鍵・kキー前置）

### 前置シフトにおけるシフトバランスの重要性

前置シフトでは、Layer 1とLayer 2は**どちらも同じ2打鍵**のコスト。
同時押しシフトと異なり、「中頻度・低頻度」の区別は打鍵効率に影響しない。

**重要なのはバランス:**
- ☆シフト(d)と★シフト(k)の使用頻度を均等化
- 片方のシフトに偏ると、特定の手に負荷集中
- `--w-shift-balance` で調整可能（デフォルト3.0）

**計算式:** `100 × min(L1, L2) / max(L1, L2)`
- 100% = 完全均等（50:50）
- 0% = 片方のみ使用

各レイヤーは3行×10列の30キー。

## 評価メトリクス

### Fitness計算式

```
Fitness = Core × (1 + Bonus / 2600)
```

**Core** = 重み付き幾何平均（低スコアが全体を大きく下げる）  
**Bonus** = 重み付き加算（高スコアで追加ボーナス）

---

### Core Metrics（乗算・必須）

低スコアは致命的ペナルティ。全体の品質を決定する必須条件。

| Metric | 指数 | 説明 | 計算方法 |
|--------|------|------|----------|
| **同指連続低** | ^1.80 | 同じ指での連続打鍵を避ける | `100 - (SFB bigrams / total bigrams × 100)` |
| **段飛ばし少** | ^1.55 | 1段飛ばしの打鍵を避ける | `100 - (row_skip bigrams / total × 100)` |
| **ホームポジ率** | ^1.30 | 中段キー使用率 | `home_row chars / total × 100` |
| **総打鍵コスト少** | ^1.05 | Norman重み付き打鍵負担 | `100 - weighted_cost` |
| **左右交互** | ^0.80 | 左右手の交互使用率 | `alternating bigrams / total × 100` |
| **単打鍵率** | ^0.70 | シフト無し打鍵率 | `layer0 chars / total × 100` |
| **Colemak類似** | ^0.60 | 音素レベルでの配置類似度 | 母音・子音の位置一致度 |

**Core計算:**
```
core_product = Π(metric_i / 100)^weight_i
Core = core_product^(1/Σweight) × 100
```

---

### Bonus Metrics（加算・奨励）

高スコアでボーナス追加。低くてもペナルティは小さい。

| Metric | 重み | 説明 | 計算方法 |
|--------|------|------|----------|
| **リダイレクト少** | ×5.0 | 同手3連打での方向転換回避 | `100 - (redirect trigrams / total × 100)` |
| **月配列類似** | ×4.0 | 月配列2-263との類似度 | 位置一致度 |
| **ロール率** | ×5.0 | 同手での流れるような連打 | `roll trigrams / total × 100` |
| **インロール** | ×5.0 | 外→内（小指→人差指）の流れ | `inroll trigrams / total × 100` |
| **アルペジオ** | ×5.0 | 隣接指の連続打鍵 | `arpeggio trigrams / total × 100` |
| **覚えやすさ** | ×2.0 | レイヤー間の一貫性 | 母音・子音の層分離度 |

**Bonus計算:**
```
Bonus = Σ(metric_i × weight_i)
```

---

## n-gramファイル形式

```
count\tcharacters\tn
```

例（1gram.txt）:
```
74569	い	1
59235	う	1
58709	ん	1
```

- `〓`（改行マーカー）は自動除外
- 2-4gramでは`〓`を含むパターンも除外

## 出力ファイル

最適化完了時に `ga_layout_seed{SEED}_{TIMESTAMP}.json` を出力。
Svelte Keyboard Analyzerで直接読み込み可能。

### JSON構造

```json
{
  "name": "GA Optimized Layout",
  "fitness": 95.23,
  "type": "ansi",
  "keys": [...],
  "legend": [...],
  "conversion": {
    "あ": { "key": "a0", "shift": [] },
    "か": { "key": "s1", "shift": ["d"] }
  },
  "arpeggio": [...]
}
```

## アーキテクチャ

```
┌─────────────────────────────────────────────────────────┐
│                    main.rs                               │
├─────────────────────────────────────────────────────────┤
│  Args (CLI)                                              │
│    ↓                                                     │
│  CorpusStats (n-gram読込 or テキスト解析)                │
│    ↓                                                     │
│  [単一実行]              [マルチラン実行]                │
│       ↓                        ↓                         │
│  Evaluator            Rayon並列(CPUコア数)               │
│       ↓                   ↓    ↓    ↓    ↓              │
│  GA実行              GA1  GA2  GA3  GA4 ...              │
│       ↓                   ↓    ↓    ↓    ↓              │
│  TUI表示             統計分析(平均/標準偏差)             │
│       ↓                        ↓                         │
│  JSON出力                 最良選択                       │
│                               ↓                          │
│                          JSON出力                        │
│                                                          │
│  GeneticAlgorithm / GpuGeneticAlgorithm:                 │
│    ├─ 初期集団生成 (30% smart + 70% random)              │
│    ├─ 評価 (GPU: bigram並列 / CPU: trigram)              │
│    ├─ 選択 (トーナメント)                                │
│    ├─ 交叉 (1点/2点/一様)                               │
│    └─ 突然変異 (swap/rotate/smart)                       │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│  gpu.rs (wgpu)           │  tui.rs (ratatui)            │
├──────────────────────────┼──────────────────────────────┤
│  - Bigram並列評価        │  - 進捗バー (Gen 0-Max)      │
│  - Layout Buffer転送     │  - Fitnessグラフ (Y: 0-100)  │
│  - 非同期実行            │  - 配列リアルタイム表示      │
│                          │  - 単一実行モードのみ        │
└──────────────────────────┴──────────────────────────────┘
```

## 参考配列

### 月配列 2-263
Colemak類似度と月配列類似度の基準。

### Colemak
音素（母音: aeiou / 子音: k,s,t,n,h,m,y,r,w）の配置パターン参照。

## 最高性能モード

### 32コアフル活用（推奨）

```bash
# 標準・高品質配列生成（10-20分）
./target/release/ga_layout_optimizer \
  --gram1 ../1gram.txt --gram2 ../2gram.txt \
  --gram3 ../3gram.txt --gram4 ../4gram.txt \
  -p 1000 -g 2000 \
  --multi-run 32 \
  --no-gpu

# 最高性能モード（30-60分）
./target/release/ga_layout_optimizer \
  --gram1 ../1gram.txt --gram2 ../2gram.txt \
  --gram3 ../3gram.txt --gram4 ../4gram.txt \
  -p 3000 -g 10000 \
  --multi-run 32 \
  --no-gpu

# 極限モード（2-4時間・Fitness 90+期待）
./target/release/ga_layout_optimizer \
  --gram1 ../1gram.txt --gram2 ../2gram.txt \
  --gram3 ../3gram.txt --gram4 ../4gram.txt \
  -p 5000 -g 20000 \
  --multi-run 32 \
  --no-gpu
```

**性能データ:**
- 32並列 × 3000個体 × 10000世代 = 9.6億評価
- メモリ使用: 20-30GB
- 期待Fitness: 85-95点
- TUI対応: TTY環境で自動起動、全実行の最良配列をリアルタイム表示

## 開発

```bash
# ビルド
cargo build --release

# テスト（短時間）
cargo run --release -- -p 50 -g 10

# 本番実行（マルチラン）
cargo run --release -- \
  --gram1 ../1gram.txt --gram2 ../2gram.txt \
  --gram3 ../3gram.txt --gram4 ../4gram.txt \
  -p 1000 -g 2000 --multi-run 16
```

## ライセンス

MIT
