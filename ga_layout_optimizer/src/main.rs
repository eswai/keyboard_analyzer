//! Genetic Algorithm based Japanese Keyboard Layout Optimizer
//! GPU-accelerated fitness evaluation with 10 evaluation metrics

mod gpu;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use unicode_normalization::UnicodeNormalization;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input corpus file for evaluation
    #[arg(short, long, default_value = "../sample.txt")]
    corpus: PathBuf,

    /// Population size
    #[arg(short, long, default_value_t = 500)]
    population: usize,

    /// Number of generations
    #[arg(short, long, default_value_t = 1000)]
    generations: usize,

    /// Mutation rate (0.0 - 1.0)
    #[arg(short, long, default_value_t = 0.15)]
    mutation_rate: f64,

    /// Elite count (individuals preserved each generation)
    #[arg(short, long, default_value_t = 10)]
    elite: usize,

    /// Random seed for reproducibility
    #[arg(short, long, default_value_t = 42)]
    seed: u64,

    /// Output file for best layout
    #[arg(short, long, default_value = "best_layout.json")]
    output: PathBuf,

    /// Disable GPU acceleration (GPU is used by default if available)
    #[arg(long, default_value_t = false)]
    no_gpu: bool,

    /// Number of parallel threads (0 = auto)
    #[arg(short, long, default_value_t = 0)]
    threads: usize,
}

// ============================================================================
// Constants: Keyboard Layout Structure
// ============================================================================

/// Key positions: 3 rows x 10 columns (excluding pinky outer columns for main chars)
const ROWS: usize = 3;
const COLS: usize = 10;
const KEYS_PER_LAYER: usize = ROWS * COLS;
const NUM_LAYERS: usize = 3; // No shift, A shift, B shift

/// Norman key weight scores (effort cost per key position)
const NORMAN_WEIGHTS: [[f64; COLS]; ROWS] = [
    [4.0, 2.0, 2.0, 3.0, 4.0, 5.0, 3.0, 2.0, 2.0, 4.0], // Top row
    [1.5, 1.0, 1.0, 1.0, 3.0, 3.0, 1.0, 1.0, 1.0, 1.5], // Home row
    [4.0, 4.0, 3.0, 2.0, 5.0, 3.0, 2.0, 3.0, 4.0, 4.0], // Bottom row
];

/// My custom key weight scores
const MY_WEIGHTS: [[f64; COLS]; ROWS] = [
    [3.5, 2.0, 2.0, 2.0, 3.0, 3.0, 2.0, 2.0, 2.0, 3.5], // Top row
    [1.5, 1.0, 1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.5], // Home row
    [3.5, 2.0, 2.0, 2.0, 3.0, 3.0, 2.0, 2.0, 2.0, 3.5], // Bottom row
];

/// Home position mask (true = home position key)
const HOME_POSITIONS: [[bool; COLS]; ROWS] = [
    [false, false, false, false, false, false, false, false, false, false],
    [true, true, true, true, false, false, true, true, true, true],
    [false, false, false, false, false, false, false, false, false, false],
];

/// Finger assignment for each key (0-4: left hand, 5-9: right hand)
const FINGER_MAP: [[u8; COLS]; ROWS] = [
    [0, 1, 2, 3, 3, 6, 6, 7, 8, 9],
    [0, 1, 2, 3, 3, 6, 6, 7, 8, 9],
    [0, 1, 2, 3, 3, 6, 6, 7, 8, 9],
];

/// Layer shift weights
const SHIFT_A_WEIGHT: f64 = 3.0;
const SHIFT_B_WEIGHT: f64 = 3.0;
const SHIFT_A_OUTER: f64 = 10.0;
const SHIFT_B_OUTER: f64 = 9.0;
const SHIFT_A_VERTICAL: f64 = 31.0;
const SHIFT_B_VERTICAL: f64 = 29.0;

// Evaluation weights
// Multiplicative core weights (these form geometric mean)
// ============================================================================
// Core Metrics (乗算・幾何平均) - 低スコアが全体を大きく下げる必須条件
// ============================================================================
const WEIGHT_ROW_SKIP: f64 = 1.4;        // 段飛ばし少: 同指で2段以上跳ぶのを避ける
const WEIGHT_SAME_FINGER: f64 = 1.0;     // 同指連続低: 同じ指の連打を避ける (SFB)
const WEIGHT_TOTAL_KEYSTROKES: f64 = 1.2; // 総打鍵少: 打鍵コスト(距離・負担)を最小化
const WEIGHT_REDIRECT_LOW: f64 = 1.1;    // リダイレクト少: 同手3連打で方向転換を避ける
const WEIGHT_COLEMAK_SIMILARITY: f64 = 0.3; // Colemak類似: 母音・子音の配置パターン
const WEIGHT_HOME_POSITION: f64 = 0.3;   // ホームポジ率: ホーム段の使用率
const WEIGHT_SINGLE_KEY: f64 = 0.4;      // 単打鍵率: シフト無しで打てる文字の割合

// ============================================================================
// Bonus Metrics (加算) - 高スコアでボーナス、低くてもペナルティ小
// ============================================================================
const WEIGHT_ALTERNATING: f64 = 7.0;     // 左右交互: 左右の手を交互に使う
const WEIGHT_TSUKI_SIMILARITY: f64 = 4.0;   // 月配列類似: 日本語に最適化された配置
const WEIGHT_ROLL: f64 = 5.0;            // ロール率: 同手で流れるような連打
const WEIGHT_INROLL: f64 = 5.0;          // インロール: 外→内への流れ (pinky→index)
const WEIGHT_ARPEGGIO: f64 = 5.0;        // アルペジオ: 隣接指の連続 (ピアノ的)
const WEIGHT_MEMORABILITY: f64 = 1.0;    // 覚えやすさ: 母音/子音のレイヤー一貫性

// ============================================================================
// Character Frequency Data
// ============================================================================

/// Hiragana characters in frequency order (most frequent first)
/// Note: 、。ー are handled separately as fixed punctuation
const HIRAGANA_FREQ: &[char] = &[
    // High frequency (unshifted layer candidates) - 28 chars for 30 positions (minus 、。)
    'い', 'う', 'ん', 'し', 'か', 'の', 'と', 'た', 'て', 'く',
    'な', 'に', 'き', 'は', 'こ', 'る', 'が', 'で', 'っ', 'す',
    'ま', 'り', 'も', 'つ', 'お', 'ら', 'を', 'さ',
    // Medium frequency (A shift layer candidates) - 30 chars
    'あ', 'れ', 'だ', 'ち', 'せ', 'け', 'ー', 'よ', 'ど', 'じ',
    'そ', 'え', 'わ', 'み', 'め', 'ひ', 'ば', 'や', 'ろ', 'ほ',
    'ふ', 'ぶ', 'ね', 'ご', 'ぎ', 'げ', 'む', 'ず', 'び', 'ざ',
    // Low frequency (B shift layer candidates) - remaining chars
    'ぐ', 'ぜ', 'へ', 'べ', 'ゆ', 'ぼ', 'ぷ', 'ぞ', 'ぱ', 'ぽ',
    'づ', 'ぴ', 'ぬ', 'ぺ', 'ゔ', 'ぢ',
    // Small kana (rare, B shift)
    'ょ', 'ゅ', 'ゃ', 'ぃ', 'ぇ', 'ぁ', 'ぉ', 'ぅ',
];

/// Characters that should be on the unshifted layer (most frequent)
const UNSHIFTED_CHARS: &[char] = &[
    'い', 'う', 'ん', 'し', 'か', 'の', 'と', 'た', 'て', 'く',
    'な', 'に', 'き', 'は', 'こ', 'る', '、', '。',
];

/// Fixed characters (punctuation) - positions in the unshifted layer
const FIXED_PUNCTUATION: &[(char, usize, usize)] = &[
    // ('、', 2, 7), // row 2, col 7
    // ('。', 2, 8), // row 2, col 8
];

/// Colemak vowel positions (for similarity comparison)
/// Colemak: Q W F P G J L U Y ;
///          A R S T D H N E I O
///          Z X C V B K M , . /
/// Vowels: A(home left pinky), E(home right middle), I(home right ring), O(home right pinky), U(top right middle)
/// Colemak vowel positions (Japanese mapping)
/// Note: ★(right shift) is at (1,7), so:
/// - 「ん」should be at (1,6) - inside ★ (N position in Colemak)
/// - 「い」should be at (1,8) - outside ★ (I position in Colemak)
const COLEMAK_VOWELS: &[(char, usize, usize)] = &[
    ('あ', 1, 0), // A position (left pinky home)
    ('い', 1, 8), // I position (right ring, outside ★)
    ('う', 0, 7), // U position (top row)
    ('え', 0, 2), // E position (top row, Colemak E)
    ('お', 1, 9), // O position (right pinky home)
];

/// Colemak consonant positions (Japanese high-frequency mapping)
/// Critical: 「ん」at N position (1,6) - inside ★
const COLEMAK_CONSONANTS: &[(char, usize, usize)] = &[
    ('ん', 1, 6), // N position - 最重要: ★の内側
    ('し', 1, 1), // S position (left ring home)
    ('た', 1, 3), // T position (left index home)
    ('の', 0, 8), // Right ring top
    ('と', 0, 3), // Left index top (F in Colemak -> T)
    ('て', 0, 6), // Right index inner top
    ('か', 0, 1), // W position -> K sound
    ('は', 0, 5), // Right index outer top -> H
    ('な', 2, 6), // N row bottom -> na
    ('く', 0, 9), // Right pinky top
];

// ============================================================================
// Data Structures
// ============================================================================

/// Represents a keyboard layout with 3 layers
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout {
    /// Layer 0: No shift, Layer 1: A shift, Layer 2: B shift
    /// Each layer: [row][col] -> character
    pub layers: [[[char; COLS]; ROWS]; NUM_LAYERS],
    /// Cached fitness score
    #[serde(skip)]
    pub fitness: f64,
    /// Detailed scores for each metric
    #[serde(skip)]
    pub scores: EvaluationScores,
}

/// Detailed evaluation scores
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EvaluationScores {
    pub row_skip: f64,
    pub home_position: f64,
    pub total_keystrokes: f64,
    pub same_finger: f64,
    pub single_key: f64,
    pub shift_chars: f64,
    pub colemak_similarity: f64,
    pub tsuki_similarity: f64,
    pub memorability: f64,
    pub alternating: f64,
    /// Roll rate: smooth finger flow on same hand (inward/outward rolls) - higher is better
    pub roll: f64,
    /// Redirect rate: direction change on same hand - lower is better (stored as 100 - redirect%)
    pub redirect_low: f64,
    /// Inroll rate: outside-to-inside rolls (pinky→index direction) - higher is better
    pub inroll: f64,
    /// Arpeggio rate: adjacent finger sequences - higher is better  
    pub arpeggio: f64,
}

/// Key position
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyPos {
    pub layer: usize,
    pub row: usize,
    pub col: usize,
}

impl KeyPos {
    pub fn new(layer: usize, row: usize, col: usize) -> Self {
        Self { layer, row, col }
    }
    
    pub fn finger(&self) -> u8 {
        FINGER_MAP[self.row][self.col]
    }
    
    pub fn is_left_hand(&self) -> bool {
        self.finger() < 5
    }
    
    pub fn is_home(&self) -> bool {
        HOME_POSITIONS[self.row][self.col]
    }
    
    pub fn weight(&self) -> f64 {
        let base = MY_WEIGHTS[self.row][self.col];
        match self.layer {
            0 => base,
            1 => base + SHIFT_A_WEIGHT,
            2 => base + SHIFT_B_WEIGHT,
            _ => base,
        }
    }
}

/// Character frequency counter from corpus
#[derive(Debug)]
pub struct CorpusStats {
    pub char_freq: HashMap<char, usize>,
    pub bigram_freq: HashMap<(char, char), usize>,
    pub trigram_freq: HashMap<(char, char, char), usize>,
    pub total_chars: usize,
}

impl CorpusStats {
    pub fn from_text(text: &str) -> Self {
        let mut char_freq: HashMap<char, usize> = HashMap::new();
        let mut bigram_freq: HashMap<(char, char), usize> = HashMap::new();
        let mut trigram_freq: HashMap<(char, char, char), usize> = HashMap::new();
        let mut total_chars = 0;
        
        let normalized: String = text.nfkc().collect();
        let chars: Vec<char> = normalized.chars()
            .filter(|c| is_hiragana(*c) || *c == '、' || *c == '。' || *c == 'ー')
            .collect();
        
        for &c in &chars {
            *char_freq.entry(c).or_insert(0) += 1;
            total_chars += 1;
        }
        
        for window in chars.windows(2) {
            let bigram = (window[0], window[1]);
            *bigram_freq.entry(bigram).or_insert(0) += 1;
        }
        
        // Trigram for roll/redirect analysis
        for window in chars.windows(3) {
            let trigram = (window[0], window[1], window[2]);
            *trigram_freq.entry(trigram).or_insert(0) += 1;
        }
        
        Self { char_freq, bigram_freq, trigram_freq, total_chars }
    }
}

/// Tsuki (月) layout reference for similarity comparison
pub struct TsukiLayout {
    pub char_positions: HashMap<char, KeyPos>,
}

impl TsukiLayout {
    pub fn new() -> Self {
        // 月配列2-263 の配置
        let mut positions = HashMap::new();
        
        // Layer 0 (unshifted) - 中指シフトキー位置は d, k
        let layer0 = [
            ['そ', 'こ', 'し', 'て', 'ょ', 'つ', 'ん', 'い', 'の', 'り'],
            ['は', 'か', '☆', 'と', 'た', 'く', 'う', '★', '゛', 'き'],
            ['す', 'け', 'に', 'な', 'さ', 'っ', 'る', '、', '。', '゜'],
        ];
        
        // Layer 1 (d shift)
        let layer1 = [
            ['ぁ', 'ひ', 'ほ', 'ふ', 'め', 'ぬ', 'え', 'み', 'や', 'ぇ'],
            ['ぃ', 'を', 'ら', 'あ', 'よ', 'ま', 'お', 'も', 'わ', 'ゆ'],
            ['ぅ', 'へ', 'せ', 'ゅ', 'ゃ', 'む', 'ろ', 'ね', 'ー', 'ぉ'],
        ];
        
        for (row, chars) in layer0.iter().enumerate() {
            for (col, &c) in chars.iter().enumerate() {
                if c != '☆' && c != '★' && c != '゛' && c != '゜' {
                    positions.insert(c, KeyPos::new(0, row, col));
                }
            }
        }
        
        for (row, chars) in layer1.iter().enumerate() {
            for (col, &c) in chars.iter().enumerate() {
                positions.insert(c, KeyPos::new(1, row, col));
            }
        }
        
        Self { char_positions: positions }
    }
}

// ============================================================================
// Layout Generation and Genetic Operators
// ============================================================================

impl Layout {
    /// Create a random layout with all required characters
    /// Fixed shift keys and punctuation at standard positions
    pub fn random(rng: &mut ChaCha8Rng) -> Self {
        let mut chars: Vec<char> = HIRAGANA_FREQ.to_vec();
        // Note: ☆★、。;・ are placed at fixed positions, not in the shuffle pool
        
        // We have 90 positions, 6 are fixed (2 shift keys + 4 punctuation)
        let total_positions = KEYS_PER_LAYER * NUM_LAYERS - 6;
        while chars.len() < total_positions {
            chars.push('　'); // Placeholder
        }
        
        chars.shuffle(rng);
        
        let mut layers = [[['　'; COLS]; ROWS]; NUM_LAYERS];
        
        // Place fixed characters
        // Layer 0 (No Shift)
        layers[0][1][2] = '☆';
        layers[0][1][7] = '★';
        layers[0][2][7] = '、';
        layers[0][2][8] = '。';
        // Layer 1 (A Shift)
        layers[1][2][7] = ';';
        layers[1][2][8] = '・';
        
        let mut idx = 0;
        for layer in 0..NUM_LAYERS {
            for row in 0..ROWS {
                for col in 0..COLS {
                    // Skip fixed positions
                    if Self::is_fixed_position(layer, row, col) {
                        continue;
                    }
                    if idx < chars.len() {
                        layers[layer][row][col] = chars[idx];
                        idx += 1;
                    }
                }
            }
        }
        
        Self {
            layers,
            fitness: 0.0,
            scores: EvaluationScores::default(),
        }
    }
    
    /// Create a smart initial layout based on frequency
    /// Places most frequent characters on unshifted layer, home positions first
    /// Fixes shift keys and punctuation at standard positions
    pub fn smart_init(rng: &mut ChaCha8Rng) -> Self {
        let mut layers = [[['　'; COLS]; ROWS]; NUM_LAYERS];
        
        // Fixed positions on No Shift layer (layer 0):
        layers[0][1][2] = '☆';  // Left shift (d key position)
        layers[0][1][7] = '★';  // Right shift (k key position)
        layers[0][2][7] = '、';
        layers[0][2][8] = '。';
        
        // Fixed positions on A Shift layer (layer 1):
        layers[1][2][7] = ';';
        layers[1][2][8] = '・';
        
        // Get hiragana chars (excluding shift keys and punctuation)
        let hiragana: Vec<char> = HIRAGANA_FREQ.to_vec();
        
        // Priority positions for unshifted layer (excluding fixed positions)
        // Home row first (best positions), then top row, then bottom row
        let unshifted_positions: Vec<(usize, usize)> = vec![
            // Home row - excluding col 2 and 7 (shift keys)
            (1, 3), (1, 6),                   // Index fingers (best after shift keys)
            (1, 1), (1, 8),                   // Ring fingers
            (1, 0), (1, 9),                   // Pinky
            // Top row
            (0, 2), (0, 3), (0, 6), (0, 7),
            (0, 1), (0, 8),
            (0, 0), (0, 9),
            // Bottom row (excluding 7, 8 which are fixed for punctuation)
            (2, 2), (2, 3), (2, 6),
            (2, 1),
            (2, 0), (2, 9),
            // Index finger stretch positions
            (1, 4), (1, 5),
            (0, 4), (0, 5),
            (2, 4), (2, 5),
        ];
        
        // Place high-frequency characters on unshifted layer (26 positions available)
        let mut char_idx = 0;
        for &(row, col) in &unshifted_positions {
            if char_idx < hiragana.len() && char_idx < 26 {
                layers[0][row][col] = hiragana[char_idx];
                char_idx += 1;
            }
        }
        
        // A shift layer (medium frequency) - 28 positions available (30 - 2 fixed)
        let shift_a_start = 26;
        let shift_a_end = (shift_a_start + 28).min(hiragana.len());
        let mut a_idx = 0;
        for row in 0..ROWS {
            for col in 0..COLS {
                // Skip fixed positions on layer 1
                if row == 2 && (col == 7 || col == 8) {
                    continue;
                }
                let src_idx = shift_a_start + a_idx;
                if src_idx < shift_a_end {
                    layers[1][row][col] = hiragana[src_idx];
                    a_idx += 1;
                }
            }
        }
        
        // B shift layer (low frequency + small kana)
        let shift_b_start = shift_a_end;
        let mut b_idx = 0;
        for row in 0..ROWS {
            for col in 0..COLS {
                let src_idx = shift_b_start + b_idx;
                if src_idx < hiragana.len() {
                    layers[2][row][col] = hiragana[src_idx];
                    b_idx += 1;
                }
            }
        }
        
        let mut layout = Self {
            layers,
            fitness: 0.0,
            scores: EvaluationScores::default(),
        };
        
        // Apply small mutation to add variety (but protect fixed positions)
        layout.mutate_smart(rng, 0.05);
        layout.repair(rng);
        layout
    }
    
    /// Smart mutation that avoids swapping fixed punctuation
    pub fn mutate_smart(&mut self, rng: &mut ChaCha8Rng, rate: f64) {
        let num_swaps = ((KEYS_PER_LAYER * NUM_LAYERS) as f64 * rate) as usize;
        
        for _ in 0..num_swaps {
            // Pick two random positions, avoiding fixed punctuation
            let (l1, r1, c1) = loop {
                let l = rng.gen_range(0..NUM_LAYERS);
                let r = rng.gen_range(0..ROWS);
                let c = rng.gen_range(0..COLS);
                // Skip fixed punctuation positions on layer 0
                if !(l == 0 && r == 2 && (c == 7 || c == 8)) {
                    break (l, r, c);
                }
            };
            
            let (l2, r2, c2) = loop {
                let l = rng.gen_range(0..NUM_LAYERS);
                let r = rng.gen_range(0..ROWS);
                let c = rng.gen_range(0..COLS);
                if !(l == 0 && r == 2 && (c == 7 || c == 8)) {
                    break (l, r, c);
                }
            };
            
            // Don't swap punctuation characters
            let c1_char = self.layers[l1][r1][c1];
            let c2_char = self.layers[l2][r2][c2];
            if c1_char != '、' && c1_char != '。' && c2_char != '、' && c2_char != '。' {
                self.layers[l1][r1][c1] = c2_char;
                self.layers[l2][r2][c2] = c1_char;
            }
        }
    }
    
    /// Find character position in layout
    pub fn find_char(&self, c: char) -> Option<KeyPos> {
        for layer in 0..NUM_LAYERS {
            for row in 0..ROWS {
                for col in 0..COLS {
                    if self.layers[layer][row][col] == c {
                        return Some(KeyPos::new(layer, row, col));
                    }
                }
            }
        }
        None
    }
    
    /// Crossover two layouts
    /// Respects fixed positions (shift keys + punctuation)
    pub fn crossover(&self, other: &Layout, rng: &mut ChaCha8Rng) -> Layout {
        let mut child = self.clone();
        
        // Use uniform crossover for each layer
        for layer in 0..NUM_LAYERS {
            if rng.gen_bool(0.5) {
                child.layers[layer] = other.layers[layer];
            }
        }
        
        // Ensure all fixed positions are preserved
        // Layer 0 (No Shift)
        child.layers[0][1][2] = '☆';
        child.layers[0][1][7] = '★';
        child.layers[0][2][7] = '、';
        child.layers[0][2][8] = '。';
        // Layer 1 (A Shift)
        child.layers[1][2][7] = ';';
        child.layers[1][2][8] = '・';
        
        // Repair: ensure no duplicate characters
        child.repair(rng);
        child
    }
    
    /// Mutation operator - respects fixed punctuation and layer hierarchy
    pub fn mutate(&mut self, rng: &mut ChaCha8Rng, rate: f64) {
        for _ in 0..((KEYS_PER_LAYER * NUM_LAYERS) as f64 * rate) as usize {
            // 70% within-layer swaps, 30% cross-layer swaps
            let same_layer = rng.gen_bool(0.7);
            
            let (l1, r1, c1) = loop {
                let l = rng.gen_range(0..NUM_LAYERS);
                let r = rng.gen_range(0..ROWS);
                let c = rng.gen_range(0..COLS);
                if !Self::is_fixed_position(l, r, c) {
                    break (l, r, c);
                }
            };
            
            let (l2, r2, c2) = loop {
                let l = if same_layer { l1 } else { rng.gen_range(0..NUM_LAYERS) };
                let r = rng.gen_range(0..ROWS);
                let c = rng.gen_range(0..COLS);
                if !Self::is_fixed_position(l, r, c) && (l != l1 || r != r1 || c != c1) {
                    break (l, r, c);
                }
            };
            
            // Don't swap punctuation characters
            let c1_char = self.layers[l1][r1][c1];
            let c2_char = self.layers[l2][r2][c2];
            if !Self::is_fixed_char(c1_char) && !Self::is_fixed_char(c2_char) {
                self.layers[l1][r1][c1] = c2_char;
                self.layers[l2][r2][c2] = c1_char;
            }
        }
    }
    
    /// Check if position is fixed (for punctuation and shift keys)
    /// Fixed positions:
    /// Layer 0 (No Shift):
    /// - row 1, col 2 (d key): Left shift ☆
    /// - row 1, col 7 (k key): Right shift ★
    /// - row 2, col 7: 、
    /// - row 2, col 8: 。
    /// Layer 1 (A Shift):
    /// - row 2, col 7: ;
    /// - row 2, col 8: ・
    fn is_fixed_position(layer: usize, row: usize, col: usize) -> bool {
        // Layer 0: shift keys and punctuation
        if layer == 0 {
            if row == 1 && (col == 2 || col == 7) {
                return true;
            }
            if row == 2 && (col == 7 || col == 8) {
                return true;
            }
        }
        // Layer 1: semicolon and middle dot
        if layer == 1 && row == 2 && (col == 7 || col == 8) {
            return true;
        }
        false
    }
    
    /// Check if character is fixed (shift keys or punctuation)
    fn is_fixed_char(c: char) -> bool {
        c == '、' || c == '。' || c == '☆' || c == '★' || c == ';' || c == '・'
    }
    
    /// Repair layout to ensure no duplicates and all required characters are present
    /// Respects fixed positions (shift keys + punctuation) and tries to keep high-freq chars on unshifted layer
    fn repair(&mut self, rng: &mut ChaCha8Rng) {
        // First, ensure all fixed characters are in place
        // Layer 0 (No Shift)
        self.layers[0][1][2] = '☆';
        self.layers[0][1][7] = '★';
        self.layers[0][2][7] = '、';
        self.layers[0][2][8] = '。';
        // Layer 1 (A Shift)
        self.layers[1][2][7] = ';';
        self.layers[1][2][8] = '・';
        
        let mut seen: HashSet<char> = HashSet::new();
        seen.insert('☆');
        seen.insert('★');
        seen.insert('、');
        seen.insert('。');
        seen.insert(';');
        seen.insert('・');
        
        let mut duplicates: Vec<(usize, usize, usize)> = Vec::new();
        let mut empty_positions: Vec<(usize, usize, usize)> = Vec::new();
        
        // Required characters: all hiragana (fixed chars already handled)
        let all_chars: HashSet<char> = HIRAGANA_FREQ.iter().copied().collect();
        
        // Find duplicates, empty positions, and used characters
        for layer in 0..NUM_LAYERS {
            for row in 0..ROWS {
                for col in 0..COLS {
                    // Skip fixed positions
                    if Self::is_fixed_position(layer, row, col) {
                        continue;
                    }
                    
                    let c = self.layers[layer][row][col];
                    if c == '　' || c == '\0' {
                        empty_positions.push((layer, row, col));
                    } else if Self::is_fixed_char(c) {
                        // Fixed char found in wrong position, mark as empty
                        empty_positions.push((layer, row, col));
                        self.layers[layer][row][col] = '　';
                    } else if seen.contains(&c) {
                        duplicates.push((layer, row, col));
                    } else {
                        seen.insert(c);
                    }
                }
            }
        }
        
        // Find missing characters and sort by frequency (high freq first)
        let mut missing: Vec<char> = all_chars.difference(&seen).copied().collect();
        // Sort by position in HIRAGANA_FREQ (lower index = higher frequency)
        missing.sort_by_key(|c| {
            HIRAGANA_FREQ.iter().position(|&x| x == *c).unwrap_or(999)
        });
        
        // Sort available positions by preference (layer 0 first, then home row)
        let mut available_positions: Vec<(usize, usize, usize)> = duplicates;
        available_positions.extend(empty_positions);
        available_positions.sort_by_key(|&(layer, row, _col)| {
            let layer_score = layer * 100;
            let row_score = if row == 1 { 0 } else { 10 }; // Prefer home row
            layer_score + row_score
        });
        
        // Replace available positions with missing characters
        // High-freq chars go to layer 0 positions first
        for (i, &c) in missing.iter().enumerate() {
            if i < available_positions.len() {
                let (layer, row, col) = available_positions[i];
                self.layers[layer][row][col] = c;
            }
        }
        
        // Fill remaining positions with placeholder (if any)
        for i in missing.len()..available_positions.len() {
            let (layer, row, col) = available_positions[i];
            self.layers[layer][row][col] = '　';
        }
    }
    
    /// Convert to JSON format
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

// ============================================================================
// Fitness Evaluation
// ============================================================================

pub struct Evaluator {
    pub corpus: CorpusStats,
    pub tsuki: TsukiLayout,
}

impl Evaluator {
    pub fn new(corpus_text: &str) -> Self {
        Self {
            corpus: CorpusStats::from_text(corpus_text),
            tsuki: TsukiLayout::new(),
        }
    }
    
    /// Evaluate a layout and return fitness score
    /// Uses hybrid multiplicative-additive approach:
    /// fitness = core_multiplier × additive_bonus
    pub fn evaluate(&self, layout: &mut Layout) -> f64 {
        let scores = self.compute_scores(layout);
        
        // ============================================================
        // Core Metrics (乗算・幾何平均): 低スコアが全体を大きく下げる
        // ============================================================
        let row_skip_norm = (scores.row_skip / 100.0).max(0.01);
        let same_finger_norm = (scores.same_finger / 100.0).max(0.01);
        let total_keystrokes_norm = (scores.total_keystrokes / 100.0).max(0.01);
        let redirect_low_norm = (scores.redirect_low / 100.0).max(0.01);
        let colemak_norm = (scores.colemak_similarity / 100.0).max(0.01);
        let home_norm = (scores.home_position / 100.0).max(0.01);
        let single_key_norm = (scores.single_key / 100.0).max(0.01);
        
        // Weighted geometric mean: (a^w1 * b^w2 * ...)^(1/sum_w)
        let total_weight = WEIGHT_ROW_SKIP + WEIGHT_SAME_FINGER + WEIGHT_TOTAL_KEYSTROKES 
            + WEIGHT_REDIRECT_LOW + WEIGHT_COLEMAK_SIMILARITY + WEIGHT_HOME_POSITION
            + WEIGHT_SINGLE_KEY;
        let core_product = 
            row_skip_norm.powf(WEIGHT_ROW_SKIP) *
            same_finger_norm.powf(WEIGHT_SAME_FINGER) *
            total_keystrokes_norm.powf(WEIGHT_TOTAL_KEYSTROKES) *
            redirect_low_norm.powf(WEIGHT_REDIRECT_LOW) *
            colemak_norm.powf(WEIGHT_COLEMAK_SIMILARITY) *
            home_norm.powf(WEIGHT_HOME_POSITION) *
            single_key_norm.powf(WEIGHT_SINGLE_KEY);
        let core_multiplier = core_product.powf(1.0 / total_weight) * 100.0;
        
        // ============================================================
        // Bonus Metrics (加算): 高スコアでボーナス、低くてもペナルティ小
        // ============================================================
        let additive_bonus = 
            scores.alternating * WEIGHT_ALTERNATING +
            scores.tsuki_similarity * WEIGHT_TSUKI_SIMILARITY +
            scores.roll * WEIGHT_ROLL +
            scores.memorability * WEIGHT_MEMORABILITY +
            scores.inroll * WEIGHT_INROLL +
            scores.arpeggio * WEIGHT_ARPEGGIO;
        
        // Final fitness: core × (1 + bonus/scale)
        let bonus_scale = 2700.0; // Adjusted: single_key to Core, shift_chars removed
        let fitness = core_multiplier * (1.0 + additive_bonus / bonus_scale);
        
        layout.scores = scores;
        layout.fitness = fitness;
        fitness
    }
    
    fn compute_scores(&self, layout: &Layout) -> EvaluationScores {
        let mut total_keystrokes = 0.0;
        let mut home_keystrokes = 0.0;
        let mut shifted_keystrokes = 0.0;
        let mut single_keystrokes = 0.0;
        let mut row_skips = 0.0;
        let mut same_finger = 0.0;
        let mut alternating = 0.0;
        let mut total_chars = 0.0;
        let mut missing_chars = 0.0; // Characters not in layout
        
        let _prev_pos: Option<KeyPos> = None;
        
        // Process corpus
        for (c, &count) in &self.corpus.char_freq {
            let count_f = count as f64;
            if let Some(pos) = layout.find_char(*c) {
                total_chars += count_f;
                
                // Total keystrokes (weighted by position)
                total_keystrokes += pos.weight() * count_f;
                
                // Home position
                if pos.is_home() {
                    home_keystrokes += count_f;
                }
                
                // Shifted characters
                if pos.layer > 0 {
                    shifted_keystrokes += count_f;
                } else {
                    single_keystrokes += count_f;
                }
            } else {
                // Penalize missing characters heavily
                missing_chars += count_f;
            }
        }
        
        // Add missing chars to total for proper ratio calculation
        let corpus_total = total_chars + missing_chars;
        
        // Process bigrams for same-finger and alternating
        let mut bigram_counted = 0.0;
        for ((c1, c2), &count) in &self.corpus.bigram_freq {
            if let (Some(pos1), Some(pos2)) = (layout.find_char(*c1), layout.find_char(*c2)) {
                let count_f = count as f64;
                bigram_counted += count_f;
                
                // Same finger penalty
                if pos1.finger() == pos2.finger() && pos1 != pos2 {
                    same_finger += count_f;
                    
                    // Row skip (段飛ばし)
                    let row_diff = (pos1.row as i32 - pos2.row as i32).abs();
                    if row_diff >= 2 {
                        row_skips += count_f;
                    }
                }
                
                // Alternating hands
                if pos1.is_left_hand() != pos2.is_left_hand() {
                    alternating += count_f;
                }
            }
        }
        
        let _bigram_total = self.corpus.bigram_freq.values().sum::<usize>() as f64;
        
        // Process trigrams for roll/redirect/inroll/arpeggio analysis
        let mut roll_count = 0.0;
        let mut redirect_count = 0.0;
        let mut inroll_count = 0.0;
        let mut arpeggio_count = 0.0;
        let mut trigram_counted = 0.0;
        
        for ((c1, c2, c3), &count) in &self.corpus.trigram_freq {
            if let (Some(pos1), Some(pos2), Some(pos3)) = (
                layout.find_char(*c1), 
                layout.find_char(*c2), 
                layout.find_char(*c3)
            ) {
                let count_f = count as f64;
                trigram_counted += count_f;
                
                let hand1 = pos1.is_left_hand();
                let hand2 = pos2.is_left_hand();
                let hand3 = pos3.is_left_hand();
                let finger1 = pos1.finger();
                let finger2 = pos2.finger();
                let finger3 = pos3.finger();
                
                // Same hand for all three keys
                if hand1 == hand2 && hand2 == hand3 {
                    // Finger index: 0=pinky, 1=ring, 2=middle, 3=index (for each hand)
                    let dir1 = finger2 as i32 - finger1 as i32;
                    let dir2 = finger3 as i32 - finger2 as i32;
                    
                    if finger1 != finger2 && finger2 != finger3 && finger1 != finger3 {
                        if (dir1 > 0 && dir2 > 0) || (dir1 < 0 && dir2 < 0) {
                            // Same direction = roll
                            roll_count += count_f;
                            
                            // Inroll: outside to inside (pinky→index = positive direction)
                            if dir1 > 0 && dir2 > 0 {
                                inroll_count += count_f;
                            }
                            
                            // Arpeggio: all adjacent fingers
                            if dir1.abs() == 1 && dir2.abs() == 1 {
                                arpeggio_count += count_f;
                            }
                        } else if (dir1 > 0 && dir2 < 0) || (dir1 < 0 && dir2 > 0) {
                            redirect_count += count_f;
                        }
                    }
                } else if hand1 != hand2 && hand2 != hand3 {
                    // Alternating pattern - handled by alternating metric
                } else {
                    // Two keys on same hand = potential roll/arpeggio
                    if hand1 == hand2 && finger1 != finger2 {
                        let dir = finger2 as i32 - finger1 as i32;
                        if dir.abs() == 1 {
                            roll_count += count_f * 0.5;
                            arpeggio_count += count_f * 0.5;
                            if dir > 0 {
                                inroll_count += count_f * 0.5;
                            }
                        }
                    }
                    if hand2 == hand3 && finger2 != finger3 {
                        let dir = finger3 as i32 - finger2 as i32;
                        if dir.abs() == 1 {
                            roll_count += count_f * 0.5;
                            arpeggio_count += count_f * 0.5;
                            if dir > 0 {
                                inroll_count += count_f * 0.5;
                            }
                        }
                    }
                }
            }
        }
        
        // Coverage penalty: heavily penalize layouts that can't type the corpus
        let coverage = if corpus_total > 0.0 { total_chars / corpus_total } else { 0.0 };
        
        // Normalize scores to 0-100 range
        // All scores are multiplied by coverage to penalize incomplete layouts
        EvaluationScores {
            // 1段飛ばしの少なさ: fewer is better
            row_skip: if bigram_counted > 0.0 { 
                100.0 * (1.0 - row_skips / bigram_counted) * coverage
            } else { 0.0 },
            
            // ホームポジション率の高さ
            home_position: if corpus_total > 0.0 { 
                100.0 * home_keystrokes / corpus_total
            } else { 0.0 },
            
            // 総打鍵数の少なさ: lower weighted cost is better
            total_keystrokes: if total_chars > 0.0 {
                let avg_weight = total_keystrokes / total_chars;
                // Best possible avg weight is ~1.0 (all home row), worst is ~5.0
                (100.0 * (1.0 - (avg_weight - 1.0) / 4.0).max(0.0)) * coverage
            } else { 0.0 },
            
            // 同指連続率の低さ
            same_finger: if bigram_counted > 0.0 { 
                100.0 * (1.0 - same_finger / bigram_counted) * coverage
            } else { 0.0 },
            
            // 単打鍵数の多さ (ratio of single keystrokes to total corpus)
            single_key: if corpus_total > 0.0 { 
                100.0 * single_keystrokes / corpus_total
            } else { 0.0 },
            
            // シフト文字数の少なさ (considering total corpus)
            shift_chars: if corpus_total > 0.0 { 
                100.0 * (corpus_total - shifted_keystrokes - missing_chars) / corpus_total
            } else { 0.0 },
            
            // Colemak類似性
            colemak_similarity: self.calc_colemak_similarity(layout),
            
            // 月配列類似性
            tsuki_similarity: self.calc_tsuki_similarity(layout),
            
            // 覚えやすさ
            memorability: self.calc_memorability(layout),
            
            // 左右交互打鍵率
            alternating: if bigram_counted > 0.0 { 
                100.0 * alternating / bigram_counted * coverage
            } else { 0.0 },
            
            // ロール率 (2/3gram運指): higher is better
            roll: if trigram_counted > 0.0 {
                100.0 * roll_count / trigram_counted * coverage
            } else { 0.0 },
            
            // リダイレクト少なさ (3gram運指): lower redirect is better, so 100 - redirect%
            redirect_low: if trigram_counted > 0.0 {
                100.0 * (1.0 - redirect_count / trigram_counted) * coverage
            } else { 0.0 },
            
            // インロール率 (外→内): higher is better
            inroll: if trigram_counted > 0.0 {
                100.0 * inroll_count / trigram_counted * coverage
            } else { 0.0 },
            
            // アルペジオ率 (隣接指連続): higher is better
            arpeggio: if trigram_counted > 0.0 {
                100.0 * arpeggio_count / trigram_counted * coverage
            } else { 0.0 },
        }
    }
    
    fn calc_colemak_similarity(&self, layout: &Layout) -> f64 {
        let mut matches = 0;
        let total = COLEMAK_VOWELS.len() + COLEMAK_CONSONANTS.len();
        
        // Check vowel positions
        for &(c, expected_row, expected_col) in COLEMAK_VOWELS {
            if let Some(pos) = layout.find_char(c) {
                if pos.layer == 0 && pos.row == expected_row && pos.col == expected_col {
                    matches += 1;
                }
            }
        }
        
        // Check consonant positions
        for &(c, expected_row, expected_col) in COLEMAK_CONSONANTS {
            if let Some(pos) = layout.find_char(c) {
                if pos.layer == 0 && pos.row == expected_row && pos.col == expected_col {
                    matches += 1;
                }
            }
        }
        
        100.0 * matches as f64 / total as f64
    }
    
    fn calc_tsuki_similarity(&self, layout: &Layout) -> f64 {
        let mut matches = 0;
        let mut total = 0;
        
        for (&c, &tsuki_pos) in &self.tsuki.char_positions {
            if let Some(pos) = layout.find_char(c) {
                total += 1;
                if pos.row == tsuki_pos.row && pos.col == tsuki_pos.col {
                    matches += 1;
                }
            }
        }
        
        if total > 0 {
            100.0 * matches as f64 / total as f64
        } else {
            0.0
        }
    }
    
    fn calc_memorability(&self, layout: &Layout) -> f64 {
        // Memorability: vowel/consonant consistency across layers
        // If a position has a vowel in layer 0, it should have a vowel in layer 1/2
        // If a position has a consonant in layer 0, it should have a consonant in layer 1/2
        
        let vowels: HashSet<char> = ['あ', 'い', 'う', 'え', 'お', 
                                      'ぁ', 'ぃ', 'ぅ', 'ぇ', 'ぉ'].iter().copied().collect();
        
        let mut consistent_positions = 0;
        let mut total_positions = 0;
        
        // Check each position (excluding fixed positions)
        for row in 0..ROWS {
            for col in 0..COLS {
                // Skip fixed positions (shift keys, punctuation)
                if Layout::is_fixed_position(0, row, col) {
                    continue;
                }
                
                let c0 = layout.layers[0][row][col];
                if c0 == '　' || c0 == '\0' {
                    continue;
                }
                
                let is_vowel_0 = vowels.contains(&c0);
                let mut layer_consistent = 0;
                let mut layer_checked = 0;
                
                // Compare with layer 1 and 2
                for layer in 1..NUM_LAYERS {
                    // Skip if this position is fixed on this layer
                    if Layout::is_fixed_position(layer, row, col) {
                        continue;
                    }
                    
                    let c_layer = layout.layers[layer][row][col];
                    if c_layer == '　' || c_layer == '\0' {
                        continue;
                    }
                    
                    layer_checked += 1;
                    let is_vowel_layer = vowels.contains(&c_layer);
                    
                    // Consistent if both are vowels or both are consonants
                    if is_vowel_0 == is_vowel_layer {
                        layer_consistent += 1;
                    }
                }
                
                if layer_checked > 0 {
                    total_positions += layer_checked;
                    consistent_positions += layer_consistent;
                }
            }
        }
        
        if total_positions > 0 {
            100.0 * consistent_positions as f64 / total_positions as f64
        } else {
            0.0
        }
    }
    
    /// Calculate trigram-based scores: roll, redirect, inroll, arpeggio
    /// Returns (roll, redirect_low, inroll, arpeggio)
    pub fn calc_trigram_scores(&self, layout: &Layout) -> (f64, f64, f64, f64) {
        let mut roll_count = 0.0;
        let mut redirect_count = 0.0;
        let mut inroll_count = 0.0;   // Outside to inside (pinky→index direction)
        let mut arpeggio_count = 0.0; // Adjacent finger sequences
        let mut trigram_counted = 0.0;
        
        for ((c1, c2, c3), &count) in &self.corpus.trigram_freq {
            if let (Some(pos1), Some(pos2), Some(pos3)) = (
                layout.find_char(*c1), 
                layout.find_char(*c2), 
                layout.find_char(*c3)
            ) {
                let count_f = count as f64;
                trigram_counted += count_f;
                
                let hand1 = pos1.is_left_hand();
                let hand2 = pos2.is_left_hand();
                let hand3 = pos3.is_left_hand();
                let finger1 = pos1.finger();
                let finger2 = pos2.finger();
                let finger3 = pos3.finger();
                
                // Same hand for all three keys
                if hand1 == hand2 && hand2 == hand3 {
                    let dir1 = finger2 as i32 - finger1 as i32;
                    let dir2 = finger3 as i32 - finger2 as i32;
                    
                    if finger1 != finger2 && finger2 != finger3 && finger1 != finger3 {
                        if (dir1 > 0 && dir2 > 0) || (dir1 < 0 && dir2 < 0) {
                            // Roll: same direction
                            roll_count += count_f;
                            
                            // Inroll: outside to inside
                            // Left hand: finger increases = inward (pinky=0 → index=3)
                            // Right hand: finger increases = inward (pinky=0 → index=3)
                            if dir1 > 0 && dir2 > 0 {
                                inroll_count += count_f;
                            }
                            
                            // Arpeggio: all adjacent fingers
                            if dir1.abs() == 1 && dir2.abs() == 1 {
                                arpeggio_count += count_f;
                            }
                        } else if (dir1 > 0 && dir2 < 0) || (dir1 < 0 && dir2 > 0) {
                            redirect_count += count_f;
                        }
                    }
                } else if !(hand1 != hand2 && hand2 != hand3) {
                    // Two keys on same hand = potential roll/arpeggio
                    if hand1 == hand2 && finger1 != finger2 {
                        let dir = finger2 as i32 - finger1 as i32;
                        if dir.abs() == 1 {
                            roll_count += count_f * 0.5;
                            arpeggio_count += count_f * 0.5;
                            if dir > 0 {
                                inroll_count += count_f * 0.5;
                            }
                        }
                    }
                    if hand2 == hand3 && finger2 != finger3 {
                        let dir = finger3 as i32 - finger2 as i32;
                        if dir.abs() == 1 {
                            roll_count += count_f * 0.5;
                            arpeggio_count += count_f * 0.5;
                            if dir > 0 {
                                inroll_count += count_f * 0.5;
                            }
                        }
                    }
                }
            }
        }
        
        let roll = if trigram_counted > 0.0 {
            100.0 * roll_count / trigram_counted
        } else { 0.0 };
        
        let redirect_low = if trigram_counted > 0.0 {
            100.0 * (1.0 - redirect_count / trigram_counted)
        } else { 100.0 };
        
        let inroll = if trigram_counted > 0.0 {
            100.0 * inroll_count / trigram_counted
        } else { 0.0 };
        
        let arpeggio = if trigram_counted > 0.0 {
            100.0 * arpeggio_count / trigram_counted
        } else { 0.0 };
        
        (roll, redirect_low, inroll, arpeggio)
    }
}

// ============================================================================
// Genetic Algorithm
// ============================================================================

pub struct GeneticAlgorithm {
    pub population: Vec<Layout>,
    pub evaluator: Evaluator,
    pub rng: ChaCha8Rng,
    pub generation: usize,
    pub best_fitness: f64,
    pub best_layout: Layout,
}

impl GeneticAlgorithm {
    pub fn new(
        population_size: usize,
        corpus_text: &str,
        seed: u64,
    ) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let evaluator = Evaluator::new(corpus_text);
        
        // Initialize population with mix of random and smart layouts
        let mut population: Vec<Layout> = Vec::with_capacity(population_size);
        
        // 30% smart initialization, 70% random
        let smart_count = population_size * 3 / 10;
        for _ in 0..smart_count {
            population.push(Layout::smart_init(&mut rng));
        }
        for _ in smart_count..population_size {
            population.push(Layout::random(&mut rng));
        }
        
        Self {
            population,
            evaluator,
            rng,
            generation: 0,
            best_fitness: 0.0,
            best_layout: Layout::random(&mut ChaCha8Rng::seed_from_u64(seed)),
        }
    }
    
    /// Run one generation
    pub fn evolve(&mut self, mutation_rate: f64, elite_count: usize) {
        // Evaluate all individuals
        self.population.par_iter_mut().for_each(|layout| {
            let mut eval = Evaluator::new(""); // Will be replaced
            eval.corpus = CorpusStats {
                char_freq: HashMap::new(),
                bigram_freq: HashMap::new(),
                trigram_freq: HashMap::new(),
                total_chars: 0,
            };
        });
        
        // Sequential evaluation (for now, can be parallelized)
        for layout in &mut self.population {
            self.evaluator.evaluate(layout);
        }
        
        // Sort by fitness (descending)
        self.population.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
        
        // Update best
        if self.population[0].fitness > self.best_fitness {
            self.best_fitness = self.population[0].fitness;
            self.best_layout = self.population[0].clone();
        }
        
        // Create next generation
        let mut next_gen: Vec<Layout> = Vec::with_capacity(self.population.len());
        
        // Keep elites
        for i in 0..elite_count.min(self.population.len()) {
            next_gen.push(self.population[i].clone());
        }
        
        // Tournament selection and crossover
        while next_gen.len() < self.population.len() {
            let parent1 = self.tournament_select(3);
            let parent2 = self.tournament_select(3);
            
            let mut child = parent1.crossover(&parent2, &mut self.rng);
            child.mutate(&mut self.rng, mutation_rate);
            next_gen.push(child);
        }
        
        self.population = next_gen;
        self.generation += 1;
    }
    
    fn tournament_select(&mut self, size: usize) -> Layout {
        let mut best: Option<&Layout> = None;
        
        for _ in 0..size {
            let idx = self.rng.gen_range(0..self.population.len());
            let candidate = &self.population[idx];
            
            if best.is_none() || candidate.fitness > best.unwrap().fitness {
                best = Some(candidate);
            }
        }
        
        best.unwrap().clone()
    }
}

// ============================================================================
// GPU-Accelerated GA
// ============================================================================

/// Build character to index mapping
pub fn build_char_to_idx() -> (HashMap<char, u32>, HashMap<u32, char>) {
    let mut char_to_idx = HashMap::new();
    let mut idx_to_char = HashMap::new();
    
    for (i, &c) in HIRAGANA_FREQ.iter().enumerate() {
        char_to_idx.insert(c, i as u32);
        idx_to_char.insert(i as u32, c);
    }
    
    // Add punctuation (、。 are in fixed positions)
    let offset = HIRAGANA_FREQ.len() as u32;
    char_to_idx.insert('、', offset);
    idx_to_char.insert(offset, '、');
    char_to_idx.insert('。', offset + 1);
    idx_to_char.insert(offset + 1, '。');
    char_to_idx.insert('　', offset + 2); // Space placeholder
    idx_to_char.insert(offset + 2, '　');
    
    (char_to_idx, idx_to_char)
}

/// Convert Layout to GPU format
impl Layout {
    pub fn to_gpu_layout(&self, char_to_idx: &HashMap<char, u32>) -> gpu::GpuLayout {
        let mut all_chars = [0u32; 96]; // 3 x 32
        
        for layer in 0..NUM_LAYERS {
            for row in 0..ROWS {
                for col in 0..COLS {
                    let idx = layer * 30 + row * 10 + col;
                    let c = self.layers[layer][row][col];
                    all_chars[idx] = *char_to_idx.get(&c).unwrap_or(&255);
                }
            }
        }
        
        let mut chars0 = [0u32; 32];
        let mut chars1 = [0u32; 32];
        let mut chars2 = [0u32; 32];
        
        chars0.copy_from_slice(&all_chars[0..32]);
        chars1.copy_from_slice(&all_chars[32..64]);
        chars2.copy_from_slice(&all_chars[64..96]);
        
        gpu::GpuLayout {
            chars0,
            chars1,
            chars2,
        }
    }
}

/// GPU-Accelerated Genetic Algorithm
pub struct GpuGeneticAlgorithm {
    pub population: Vec<Layout>,
    pub evaluator: Evaluator,
    pub rng: ChaCha8Rng,
    pub generation: usize,
    pub best_fitness: f64,
    pub best_layout: Layout,
    
    // GPU context
    gpu_ctx: Option<gpu::GpuContext>,
    char_to_idx: HashMap<char, u32>,
    idx_to_char: HashMap<u32, char>,
}

impl GpuGeneticAlgorithm {
    pub fn new(
        population_size: usize,
        corpus_text: &str,
        seed: u64,
        use_gpu: bool,
    ) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let evaluator = Evaluator::new(corpus_text);
        
        // Build character mappings
        let (char_to_idx, idx_to_char) = build_char_to_idx();
        
        // Initialize GPU context if requested
        let gpu_ctx = if use_gpu {
            let char_freqs = gpu::prepare_char_freqs(&evaluator.corpus.char_freq, &char_to_idx);
            let bigram_freqs = gpu::prepare_bigram_freqs(&evaluator.corpus.bigram_freq, &char_to_idx);
            let key_props = gpu::prepare_key_props();
            
            match pollster::block_on(gpu::GpuContext::new(
                population_size,
                &char_freqs,
                &bigram_freqs,
                &key_props,
            )) {
                Ok(ctx) => {
                    println!("GPU context initialized successfully");
                    Some(ctx)
                }
                Err(e) => {
                    println!("Failed to initialize GPU: {}", e);
                    println!("Falling back to CPU mode");
                    None
                }
            }
        } else {
            None
        };
        
        // Initialize population
        let mut population: Vec<Layout> = Vec::with_capacity(population_size);
        let smart_count = population_size * 3 / 10;
        for _ in 0..smart_count {
            population.push(Layout::smart_init(&mut rng));
        }
        for _ in smart_count..population_size {
            population.push(Layout::random(&mut rng));
        }
        
        Self {
            population,
            evaluator,
            rng,
            generation: 0,
            best_fitness: 0.0,
            best_layout: Layout::random(&mut ChaCha8Rng::seed_from_u64(seed)),
            gpu_ctx,
            char_to_idx,
            idx_to_char,
        }
    }
    
    /// Evaluate all layouts (GPU or CPU)
    pub fn evaluate_all(&mut self) {
        if let Some(ref gpu_ctx) = self.gpu_ctx {
            // GPU evaluation
            let gpu_layouts: Vec<gpu::GpuLayout> = self.population
                .iter()
                .map(|l| l.to_gpu_layout(&self.char_to_idx))
                .collect();
            
            let results = gpu_ctx.evaluate_batch(&gpu_layouts);
            
            // Convert GPU results to fitness scores
            for (layout, result) in self.population.iter_mut().zip(results.iter()) {
                let scores = convert_gpu_result_to_scores(result, layout, &self.evaluator);
                layout.scores = scores.clone();
                layout.fitness = compute_weighted_fitness(&scores);
            }
        } else {
            // CPU evaluation (parallel)
            self.population.par_iter_mut().for_each(|layout| {
                let evaluator = Evaluator::new("");
                // Note: This is a workaround since we can't share the evaluator easily
                // In production, you'd want to use thread-local storage or Arc
            });
            
            // Sequential evaluation for correctness
            for layout in &mut self.population {
                self.evaluator.evaluate(layout);
            }
        }
    }
    
    /// Run one generation
    pub fn evolve(&mut self, mutation_rate: f64, elite_count: usize) {
        // Evaluate all
        self.evaluate_all();
        
        // Sort by fitness (descending)
        self.population.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
        
        // Update best
        if self.population[0].fitness > self.best_fitness {
            self.best_fitness = self.population[0].fitness;
            self.best_layout = self.population[0].clone();
        }
        
        // Create next generation
        let mut next_gen: Vec<Layout> = Vec::with_capacity(self.population.len());
        
        // Keep elites
        for i in 0..elite_count.min(self.population.len()) {
            next_gen.push(self.population[i].clone());
        }
        
        // Tournament selection and crossover
        while next_gen.len() < self.population.len() {
            let parent1 = self.tournament_select(3);
            let parent2 = self.tournament_select(3);
            
            let mut child = parent1.crossover(&parent2, &mut self.rng);
            child.mutate(&mut self.rng, mutation_rate);
            next_gen.push(child);
        }
        
        self.population = next_gen;
        self.generation += 1;
    }
    
    fn tournament_select(&mut self, size: usize) -> Layout {
        let mut best: Option<&Layout> = None;
        
        for _ in 0..size {
            let idx = self.rng.gen_range(0..self.population.len());
            let candidate = &self.population[idx];
            
            if best.is_none() || candidate.fitness > best.unwrap().fitness {
                best = Some(candidate);
            }
        }
        
        best.unwrap().clone()
    }
}

/// Convert GPU fitness result to evaluation scores
fn convert_gpu_result_to_scores(
    result: &gpu::GpuFitnessResult,
    layout: &Layout,
    evaluator: &Evaluator,
) -> EvaluationScores {
    let total_chars = result.total_chars as f64;
    let bigram_total = result.bigram_total as f64;
    
    // CPU calculation for trigrams (GPU hybrid mode)
    let (roll, redirect_low, inroll, arpeggio) = evaluator.calc_trigram_scores(layout);
    
    EvaluationScores {
        row_skip: if bigram_total > 0.0 {
            100.0 * (1.0 - result.row_skips as f64 / bigram_total)
        } else { 100.0 },
        
        home_position: if total_chars > 0.0 {
            100.0 * result.home_keystrokes as f64 / total_chars
        } else { 0.0 },
        
        total_keystrokes: if total_chars > 0.0 {
            let avg_weight = result.total_keystrokes as f64 / total_chars;
            100.0 * (1.0 - (avg_weight - 1.0) / 4.0).max(0.0)
        } else { 0.0 },
        
        same_finger: if bigram_total > 0.0 {
            100.0 * (1.0 - result.same_finger as f64 / bigram_total)
        } else { 100.0 },
        
        single_key: if total_chars > 0.0 {
            100.0 * result.single_keystrokes as f64 / total_chars
        } else { 0.0 },
        
        shift_chars: if total_chars > 0.0 {
            100.0 * (1.0 - result.shifted_keystrokes as f64 / total_chars)
        } else { 100.0 },
        
        // These require CPU calculation
        colemak_similarity: evaluator.calc_colemak_similarity(layout),
        tsuki_similarity: evaluator.calc_tsuki_similarity(layout),
        memorability: evaluator.calc_memorability(layout),
        
        alternating: if bigram_total > 0.0 {
            100.0 * result.alternating as f64 / bigram_total
        } else { 0.0 },
        
        // Trigram metrics from CPU calculation
        roll,
        redirect_low,
        inroll,
        arpeggio,
    }
}

/// Compute weighted fitness from scores using hybrid multiplicative-additive approach
fn compute_weighted_fitness(scores: &EvaluationScores) -> f64 {
    // Core metrics
    let row_skip_norm = (scores.row_skip / 100.0).max(0.01);
    let same_finger_norm = (scores.same_finger / 100.0).max(0.01);
    let total_keystrokes_norm = (scores.total_keystrokes / 100.0).max(0.01);
    let redirect_low_norm = (scores.redirect_low / 100.0).max(0.01);
    let colemak_norm = (scores.colemak_similarity / 100.0).max(0.01);
    let home_norm = (scores.home_position / 100.0).max(0.01);
    let single_key_norm = (scores.single_key / 100.0).max(0.01);
    
    let total_weight = WEIGHT_ROW_SKIP + WEIGHT_SAME_FINGER + WEIGHT_TOTAL_KEYSTROKES 
        + WEIGHT_REDIRECT_LOW + WEIGHT_COLEMAK_SIMILARITY + WEIGHT_HOME_POSITION
        + WEIGHT_SINGLE_KEY;
    let core_product = 
        row_skip_norm.powf(WEIGHT_ROW_SKIP) *
        same_finger_norm.powf(WEIGHT_SAME_FINGER) *
        total_keystrokes_norm.powf(WEIGHT_TOTAL_KEYSTROKES) *
        redirect_low_norm.powf(WEIGHT_REDIRECT_LOW) *
        colemak_norm.powf(WEIGHT_COLEMAK_SIMILARITY) *
        home_norm.powf(WEIGHT_HOME_POSITION) *
        single_key_norm.powf(WEIGHT_SINGLE_KEY);
    let core_multiplier = core_product.powf(1.0 / total_weight) * 100.0;
    
    // Bonus metrics
    let additive_bonus = 
        scores.alternating * WEIGHT_ALTERNATING +
        scores.tsuki_similarity * WEIGHT_TSUKI_SIMILARITY +
        scores.roll * WEIGHT_ROLL +
        scores.memorability * WEIGHT_MEMORABILITY +
        scores.inroll * WEIGHT_INROLL +
        scores.arpeggio * WEIGHT_ARPEGGIO;
    
    let bonus_scale = 2700.0;
    core_multiplier * (1.0 + additive_bonus / bonus_scale)
}

// ============================================================================
// Utility Functions
// ============================================================================

fn is_hiragana(c: char) -> bool {
    ('\u{3040}'..='\u{309F}').contains(&c) || c == 'ー'
}

fn format_layout(layout: &Layout) -> String {
    let mut result = String::new();
    
    for layer in 0..NUM_LAYERS {
        let layer_name = match layer {
            0 => "No Shift",
            1 => "A Shift",
            2 => "B Shift",
            _ => "Unknown",
        };
        result.push_str(&format!("\n=== {} ===\n", layer_name));
        
        for row in 0..ROWS {
            for col in 0..COLS {
                let c = layout.layers[layer][row][col];
                if col == 5 {
                    result.push_str("  ");
                }
                result.push_str(&format!("{} ", c));
            }
            result.push('\n');
        }
    }
    
    result
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() {
    let args = Args::parse();
    
    // Set thread count
    if args.threads > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(args.threads)
            .build_global()
            .unwrap();
    }
    
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   Genetic Algorithm Keyboard Layout Optimizer                 ║");
    println!("║   遺伝的アルゴリズムによる日本語キーボード配列最適化          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Configuration:");
    println!("  Population: {}", args.population);
    println!("  Generations: {}", args.generations);
    println!("  Mutation Rate: {:.2}", args.mutation_rate);
    println!("  Elite Count: {}", args.elite);
    println!("  Seed: {} (reproducible)", args.seed);
    println!("  GPU Acceleration: {}", if args.no_gpu { "Disabled (CPU mode)" } else { "Auto (hybrid mode)" });
    println!();
    
    // Load corpus
    let corpus_text = fs::read_to_string(&args.corpus)
        .expect("Failed to read corpus file");
    
    // Count hiragana characters
    let hiragana_count: usize = corpus_text.chars()
        .filter(|c| is_hiragana(*c) || *c == '、' || *c == '。')
        .count();
    println!("Loaded corpus: {} bytes, {} hiragana characters", corpus_text.len(), hiragana_count);
    
    // Use GPU-accelerated GA by default (hybrid mode with CPU trigram)
    if !args.no_gpu {
        run_gpu_ga(&args, &corpus_text);
    } else {
        run_cpu_ga(&args, &corpus_text);
    }
}

fn run_cpu_ga(args: &Args, corpus_text: &str) {
    // Initialize GA
    let mut ga = GeneticAlgorithm::new(args.population, corpus_text, args.seed);
    
    // Evaluate initial population
    for layout in &mut ga.population {
        ga.evaluator.evaluate(layout);
    }
    ga.population.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
    ga.best_fitness = ga.population[0].fitness;
    ga.best_layout = ga.population[0].clone();
    
    println!("Initial best fitness: {:.4}", ga.best_fitness);
    
    // Progress bar
    let pb = ProgressBar::new(args.generations as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} (ETA: {eta}) | Best: {msg}")
            .unwrap()
            .progress_chars("#>-")
    );
    
    // Evolution loop
    for gen in 0..args.generations {
        ga.evolve(args.mutation_rate, args.elite);
        
        pb.set_position((gen + 1) as u64);
        pb.set_message(format!("{:.4}", ga.best_fitness));
        
        // Print progress every 100 generations
        if (gen + 1) % 100 == 0 {
            print_progress(gen + 1, &ga.best_layout, ga.best_fitness);
        }
    }
    
    pb.finish_with_message(format!("{:.4}", ga.best_fitness));
    print_final_results(&ga.best_layout, ga.best_fitness, args);
}

fn run_gpu_ga(args: &Args, corpus_text: &str) {
    // Initialize GPU-accelerated GA
    let mut ga = GpuGeneticAlgorithm::new(args.population, corpus_text, args.seed, true);
    
    // Evaluate initial population
    ga.evaluate_all();
    ga.population.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
    ga.best_fitness = ga.population[0].fitness;
    ga.best_layout = ga.population[0].clone();
    
    println!("Initial best fitness: {:.4}", ga.best_fitness);
    
    // Progress bar
    let pb = ProgressBar::new(args.generations as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} (ETA: {eta}) | Best: {msg}")
            .unwrap()
            .progress_chars("#>-")
    );
    
    // Evolution loop
    for gen in 0..args.generations {
        ga.evolve(args.mutation_rate, args.elite);
        
        pb.set_position((gen + 1) as u64);
        pb.set_message(format!("{:.4}", ga.best_fitness));
        
        // Print progress every 100 generations
        if (gen + 1) % 100 == 0 {
            print_progress(gen + 1, &ga.best_layout, ga.best_fitness);
        }
    }
    
    pb.finish_with_message(format!("{:.4}", ga.best_fitness));
    print_final_results(&ga.best_layout, ga.best_fitness, args);
}

fn print_progress(gen: usize, layout: &Layout, fitness: f64) {
    println!("\n\nGeneration {}: Best Fitness = {:.4}", gen, fitness);
    let s = &layout.scores;
    
    // Compute core multiplier
    let row_skip_norm = (s.row_skip / 100.0).max(0.01);
    let same_finger_norm = (s.same_finger / 100.0).max(0.01);
    let total_keystrokes_norm = (s.total_keystrokes / 100.0).max(0.01);
    let redirect_low_norm = (s.redirect_low / 100.0).max(0.01);
    let colemak_norm = (s.colemak_similarity / 100.0).max(0.01);
    let home_norm = (s.home_position / 100.0).max(0.01);
    let single_key_norm = (s.single_key / 100.0).max(0.01);
    let total_weight = WEIGHT_ROW_SKIP + WEIGHT_SAME_FINGER + WEIGHT_TOTAL_KEYSTROKES 
        + WEIGHT_REDIRECT_LOW + WEIGHT_COLEMAK_SIMILARITY + WEIGHT_HOME_POSITION
        + WEIGHT_SINGLE_KEY;
    let core_product = row_skip_norm.powf(WEIGHT_ROW_SKIP) 
        * same_finger_norm.powf(WEIGHT_SAME_FINGER) 
        * total_keystrokes_norm.powf(WEIGHT_TOTAL_KEYSTROKES)
        * redirect_low_norm.powf(WEIGHT_REDIRECT_LOW)
        * colemak_norm.powf(WEIGHT_COLEMAK_SIMILARITY)
        * home_norm.powf(WEIGHT_HOME_POSITION)
        * single_key_norm.powf(WEIGHT_SINGLE_KEY);
    let core_multiplier = core_product.powf(1.0 / total_weight) * 100.0;
    
    println!("Core (乗算・必須): {:.2}", core_multiplier);
    println!("  段飛ばし少(2g同指): {:.2}% ^{}", s.row_skip, WEIGHT_ROW_SKIP);
    println!("  同指連続低(2g SFB): {:.2}% ^{}", s.same_finger, WEIGHT_SAME_FINGER);
    println!("  総打鍵コスト少:   {:.2}% ^{}", s.total_keystrokes, WEIGHT_TOTAL_KEYSTROKES);
    println!("  リダイレクト少(3g): {:.2}% ^{}", s.redirect_low, WEIGHT_REDIRECT_LOW);
    println!("  Colemak類似:       {:.2}% ^{}", s.colemak_similarity, WEIGHT_COLEMAK_SIMILARITY);
    println!("  ホームポジ率:     {:.2}% ^{}", s.home_position, WEIGHT_HOME_POSITION);
    println!("  単打鍵率:         {:.2}% ^{}", s.single_key, WEIGHT_SINGLE_KEY);
    
    println!("Bonus (加算・奨励):");
    println!("  左右交互(2g):   {:.2} x {} = {:.2}", s.alternating, WEIGHT_ALTERNATING, s.alternating * WEIGHT_ALTERNATING);
    println!("  月配列類似:     {:.2} x {} = {:.2}", s.tsuki_similarity, WEIGHT_TSUKI_SIMILARITY, s.tsuki_similarity * WEIGHT_TSUKI_SIMILARITY);
    println!("  ロール率(3g):   {:.2} x {} = {:.2}", s.roll, WEIGHT_ROLL, s.roll * WEIGHT_ROLL);
    println!("  覚えやすさ:     {:.2} x {} = {:.2}", s.memorability, WEIGHT_MEMORABILITY, s.memorability * WEIGHT_MEMORABILITY);
    println!("  インロール(3g): {:.2} x {} = {:.2}", s.inroll, WEIGHT_INROLL, s.inroll * WEIGHT_INROLL);
    println!("  アルペジオ(3g): {:.2} x {} = {:.2}", s.arpeggio, WEIGHT_ARPEGGIO, s.arpeggio * WEIGHT_ARPEGGIO);
}

fn print_final_results(layout: &Layout, fitness: f64, args: &Args) {
    // Print final results
    println!("\n\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    OPTIMIZATION COMPLETE                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("\nFinal Best Fitness: {:.4}", fitness);
    println!("\nBest Layout:");
    println!("{}", format_layout(layout));
    
    let s = &layout.scores;
    
    // Compute core multiplier for display
    let row_skip_norm = (s.row_skip / 100.0).max(0.01);
    let same_finger_norm = (s.same_finger / 100.0).max(0.01);
    let total_keystrokes_norm = (s.total_keystrokes / 100.0).max(0.01);
    let redirect_low_norm = (s.redirect_low / 100.0).max(0.01);
    let colemak_norm = (s.colemak_similarity / 100.0).max(0.01);
    let home_norm = (s.home_position / 100.0).max(0.01);
    let single_key_norm = (s.single_key / 100.0).max(0.01);
    let total_core_weight = WEIGHT_ROW_SKIP + WEIGHT_SAME_FINGER + WEIGHT_TOTAL_KEYSTROKES 
        + WEIGHT_REDIRECT_LOW + WEIGHT_COLEMAK_SIMILARITY + WEIGHT_HOME_POSITION
        + WEIGHT_SINGLE_KEY;
    let core_product = row_skip_norm.powf(WEIGHT_ROW_SKIP) 
        * same_finger_norm.powf(WEIGHT_SAME_FINGER) 
        * total_keystrokes_norm.powf(WEIGHT_TOTAL_KEYSTROKES)
        * redirect_low_norm.powf(WEIGHT_REDIRECT_LOW)
        * colemak_norm.powf(WEIGHT_COLEMAK_SIMILARITY)
        * home_norm.powf(WEIGHT_HOME_POSITION)
        * single_key_norm.powf(WEIGHT_SINGLE_KEY);
    let core_multiplier = core_product.powf(1.0 / total_core_weight) * 100.0;
    
    let additive_bonus = 
        s.alternating * WEIGHT_ALTERNATING +
        s.tsuki_similarity * WEIGHT_TSUKI_SIMILARITY +
        s.roll * WEIGHT_ROLL +
        s.memorability * WEIGHT_MEMORABILITY +
        s.inroll * WEIGHT_INROLL +
        s.arpeggio * WEIGHT_ARPEGGIO;
    
    println!("\n=== Scoring: Core × (1 + Bonus/2700) ===");
    println!("\nCore Metrics (乗算・幾何平均) - 低スコアは致命的:");
    println!("┌───────────────────────────┬────────┬────────┐");
    println!("│ Metric                    │ Score  │ Weight │");
    println!("├───────────────────────────┼────────┼────────┤");
    println!("│ 段飛ばし少 (2g同指跳躍)  │ {:6.2}%│ ^{:.1}  │", s.row_skip, WEIGHT_ROW_SKIP);
    println!("│ 同指連続低 (2g SFB回避)  │ {:6.2}%│ ^{:.1}  │", s.same_finger, WEIGHT_SAME_FINGER);
    println!("│ 総打鍵コスト少 (距離負担)│ {:6.2}%│ ^{:.1}  │", s.total_keystrokes, WEIGHT_TOTAL_KEYSTROKES);
    println!("│ リダイレクト少 (3g方向)  │ {:6.2}%│ ^{:.1}  │", s.redirect_low, WEIGHT_REDIRECT_LOW);
    println!("│ Colemak類似 (母音子音配置)│ {:6.2}%│ ^{:.1}  │", s.colemak_similarity, WEIGHT_COLEMAK_SIMILARITY);
    println!("│ ホームポジ率 (中段使用)  │ {:6.2}%│ ^{:.1}  │", s.home_position, WEIGHT_HOME_POSITION);
    println!("│ 単打鍵率 (シフト無し)    │ {:6.2}%│ ^{:.1}  │", s.single_key, WEIGHT_SINGLE_KEY);
    println!("├───────────────────────────┼────────┼────────┤");
    println!("│ Core Multiplier           │ {:6.2} │        │", core_multiplier);
    println!("└───────────────────────────┴────────┴────────┘");
    
    println!("\nBonus Metrics (加算) - 高スコアで加点:");
    println!("┌───────────────────────────┬────────┬────────┬──────────┐");
    println!("│ Metric                    │ Score  │ Weight │ Weighted │");
    println!("├───────────────────────────┼────────┼────────┼──────────┤");
    println!("│ 左右交互 (2g手の切替)    │ {:6.2} │ {:6.1} │ {:8.2} │", s.alternating, WEIGHT_ALTERNATING, s.alternating * WEIGHT_ALTERNATING);
    println!("│ 月配列類似 (日本語最適)  │ {:6.2} │ {:6.1} │ {:8.2} │", s.tsuki_similarity, WEIGHT_TSUKI_SIMILARITY, s.tsuki_similarity * WEIGHT_TSUKI_SIMILARITY);
    println!("│ ロール率 (3g同手の流れ)  │ {:6.2} │ {:6.1} │ {:8.2} │", s.roll, WEIGHT_ROLL, s.roll * WEIGHT_ROLL);
    println!("│ 覚えやすさ (レイヤー一貫)│ {:6.2} │ {:6.1} │ {:8.2} │", s.memorability, WEIGHT_MEMORABILITY, s.memorability * WEIGHT_MEMORABILITY);
    println!("│ インロール (3g外→内)    │ {:6.2} │ {:6.1} │ {:8.2} │", s.inroll, WEIGHT_INROLL, s.inroll * WEIGHT_INROLL);
    println!("│ アルペジオ (3g隣接指)    │ {:6.2} │ {:6.1} │ {:8.2} │", s.arpeggio, WEIGHT_ARPEGGIO, s.arpeggio * WEIGHT_ARPEGGIO);
    println!("├───────────────────────────┼────────┼────────┼──────────┤");
    println!("│ Bonus Total               │        │        │ {:8.2} │", additive_bonus);
    println!("└───────────────────────────┴────────┴────────┴──────────┘");
    
    println!("\nFinal: {:.2} × (1 + {:.2}/2700) = {:.2}", core_multiplier, additive_bonus, fitness);
    
    // Save to file
    let s = &layout.scores;
    let output_json = serde_json::json!({
        "name": format!("GA Optimized Layout (seed: {})", args.seed),
        "fitness": fitness,
        "generation": args.generations,
        "scores": {
            "row_skip": s.row_skip,
            "home_position": s.home_position,
            "total_keystrokes": s.total_keystrokes,
            "same_finger": s.same_finger,
            "single_key": s.single_key,
            "shift_chars": s.shift_chars,
            "colemak_similarity": s.colemak_similarity,
            "tsuki_similarity": s.tsuki_similarity,
            "memorability": s.memorability,
            "alternating": s.alternating,
            "roll": s.roll,
            "redirect_low": s.redirect_low,
            "inroll": s.inroll,
            "arpeggio": s.arpeggio,
        },
        "layers": {
            "no_shift": layout.layers[0],
            "a_shift": layout.layers[1],
            "b_shift": layout.layers[2],
        }
    });
    
    fs::write(&args.output, serde_json::to_string_pretty(&output_json).unwrap())
        .expect("Failed to write output file");
    
    println!("\nLayout saved to: {}", args.output.display());
    
    // Also generate keyboard_analyzer compatible JSON
    generate_keyboard_analyzer_json(layout, args);
}

/// Generate a JSON file compatible with the existing keyboard_analyzer
fn generate_keyboard_analyzer_json(layout: &Layout, args: &Args) {
    // Key IDs in QWERTY order for 3 rows
    let key_ids = [
        ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
        ["a", "s", "d", "f", "g", "h", "j", "k", "l", ";"],
        ["z", "x", "c", "v", "b", "n", "m", ",", ".", "/"],
    ];
    
    let finger_map = [
        [0, 1, 2, 3, 3, 6, 6, 7, 8, 9],
        [0, 1, 2, 3, 3, 6, 6, 7, 8, 9],
        [0, 1, 2, 3, 3, 6, 6, 7, 8, 9],
    ];
    
    let home_mask = [
        [false, false, false, false, false, false, false, false, false, false],
        [true, true, true, true, false, false, true, true, true, true],
        [false, false, false, false, false, false, false, false, false, false],
    ];
    
    // Build keys array
    let mut keys_json = Vec::new();
    
    for row in 0..3 {
        let mut row_keys = Vec::new();
        for col in 0..10 {
            let c0 = layout.layers[0][row][col];
            let c1 = layout.layers[1][row][col];
            let c2 = layout.layers[2][row][col];
            
            let mut key = serde_json::json!({
                "id": key_ids[row][col],
                "legend": [c0.to_string(), c1.to_string(), c2.to_string()],
                "size": 1,
                "finger": finger_map[row][col],
            });
            
            if home_mask[row][col] {
                key["home"] = serde_json::json!(true);
            }
            
            row_keys.push(key);
        }
        keys_json.push(row_keys);
    }
    
    let analyzer_json = serde_json::json!({
        "name": format!("GA Optimized Layout (seed: {}, gen: {})", args.seed, args.generations),
        "remark": format!("Generated by GA optimizer. Fitness: {:.4}", layout.fitness),
        "keys": keys_json,
        "arpeggio": [],
        "conversion": {}
    });
    
    let analyzer_path = args.output.with_file_name(
        format!("ga_layout_seed{}_{}.json", args.seed, 
            chrono_lite_timestamp())
    );
    
    fs::write(&analyzer_path, serde_json::to_string_pretty(&analyzer_json).unwrap())
        .expect("Failed to write analyzer JSON");
    
    println!("Analyzer-compatible JSON saved to: {}", analyzer_path.display());
}

fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{}", duration.as_secs())
}
