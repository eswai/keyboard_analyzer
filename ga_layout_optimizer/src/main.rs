//! Genetic Algorithm based Japanese Keyboard Layout Optimizer
//! GPU-accelerated fitness evaluation with 10 evaluation metrics

mod gpu;
mod tui;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use unicode_normalization::UnicodeNormalization;

/// Command line arguments
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input corpus file for evaluation (fallback if n-gram files not provided)
    #[arg(short, long, default_value = "../sample.txt")]
    corpus: PathBuf,

    /// 1-gram file (format: count\tchar\t1)
    #[arg(long)]
    gram1: Option<PathBuf>,

    /// 2-gram file (format: count\tbigram\t2)
    #[arg(long)]
    gram2: Option<PathBuf>,

    /// 3-gram file (format: count\ttrigram\t3)
    #[arg(long)]
    gram3: Option<PathBuf>,

    /// 4-gram file (format: count\t4gram\t4)
    #[arg(long)]
    gram4: Option<PathBuf>,

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

    /// Random seed for reproducibility (only used in single run mode)
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

    /// Enable TUI mode for real-time visualization
    #[arg(long, default_value_t = false)]
    tui: bool,

    /// Number of parallel GA runs with random seeds (0 = single run, max = CPU cores)
    #[arg(long, default_value_t = 0)]
    multi_run: usize,

    // ============================================================================
    // Core Metric Weights (multiplicative, exponents)
    // ============================================================================
    /// Weight: 同指連続率の低さ (same finger avoidance)
    #[arg(long, default_value_t = 1.8)]
    w_same_finger: f64,

    /// Weight: 段越えの少なさ (row skip avoidance)
    #[arg(long, default_value_t = 1.55)]
    w_row_skip: f64,

    /// Weight: ホームポジション率の高さ (home position rate)
    #[arg(long, default_value_t = 1.3)]
    w_home_position: f64,

    /// Weight: 総打鍵数の少なさ (total keystrokes efficiency)
    #[arg(long, default_value_t = 1.05)]
    w_total_keystrokes: f64,

    /// Weight: 左右交互打鍵率の高さ (alternating hands)
    #[arg(long, default_value_t = 0.8)]
    w_alternating: f64,

    /// Weight: 単打鍵率の高さ (single keystroke rate)
    #[arg(long, default_value_t = 0.7)]
    w_single_key: f64,

    /// Weight: Colemak類似度 (Colemak similarity, phoneme-based)
    #[arg(long, default_value_t = 0.6)]
    w_colemak_similarity: f64,

    // ============================================================================
    // Bonus Metric Weights (additive)
    // ============================================================================
    /// Weight: リダイレクト少 (redirect avoidance)
    #[arg(long, default_value_t = 5.0)]
    w_redirect_low: f64,

    /// Weight: 月配列類似度 (Tsuki layout similarity)
    #[arg(long, default_value_t = 4.0)]
    w_tsuki_similarity: f64,

    /// Weight: ロール率 (roll rate)
    #[arg(long, default_value_t = 5.0)]
    w_roll: f64,

    /// Weight: インロール率 (inroll rate)
    #[arg(long, default_value_t = 5.0)]
    w_inroll: f64,

    /// Weight: アルペジオ率 (arpeggio rate)
    #[arg(long, default_value_t = 5.0)]
    w_arpeggio: f64,

    /// Weight: 覚えやすさ (memorability)
    #[arg(long, default_value_t = 2.0)]
    w_memorability: f64,

    /// Weight: シフトバランス (shift balance between Layer1 and Layer2)
    #[arg(long, default_value_t = 3.0)]
    w_shift_balance: f64,
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

// ============================================================================
// Evaluation Weights Structure
// ============================================================================
#[derive(Debug, Clone)]
pub struct EvaluationWeights {
    // Core Metrics (multiplicative, exponents)
    pub same_finger: f64,
    pub row_skip: f64,
    pub home_position: f64,
    pub total_keystrokes: f64,
    pub alternating: f64,
    pub single_key: f64,
    pub colemak_similarity: f64,
    
    // Bonus Metrics (additive)
    pub redirect_low: f64,
    pub tsuki_similarity: f64,
    pub roll: f64,
    pub inroll: f64,
    pub arpeggio: f64,
    pub memorability: f64,
    pub shift_balance: f64,  // 前置シフト用: Layer1とLayer2の使用頻度均等化
}

impl Default for EvaluationWeights {
    fn default() -> Self {
        Self {
            // Core (exponents)
            same_finger: 1.8,
            row_skip: 1.55,
            home_position: 1.3,
            total_keystrokes: 1.05,
            alternating: 0.8,
            single_key: 0.7,
            colemak_similarity: 0.6,
            
            // Bonus (additive)
            redirect_low: 5.0,
            tsuki_similarity: 4.0,
            roll: 5.0,
            inroll: 5.0,
            arpeggio: 5.0,
            memorability: 2.0,
            shift_balance: 3.0,  // 前置シフト用
        }
    }
}

impl From<&Args> for EvaluationWeights {
    fn from(args: &Args) -> Self {
        Self {
            same_finger: args.w_same_finger,
            row_skip: args.w_row_skip,
            home_position: args.w_home_position,
            total_keystrokes: args.w_total_keystrokes,
            alternating: args.w_alternating,
            single_key: args.w_single_key,
            colemak_similarity: args.w_colemak_similarity,
            
            redirect_low: args.w_redirect_low,
            tsuki_similarity: args.w_tsuki_similarity,
            roll: args.w_roll,
            inroll: args.w_inroll,
            arpeggio: args.w_arpeggio,
            memorability: args.w_memorability,
            shift_balance: args.w_shift_balance,
        }
    }
}

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

/// Colemak key positions
/// Colemak: Q W F P G J L U Y ;
///          A R S T D H N E I O
///          Z X C V B K M , . /
const COLEMAK_POSITIONS: &[(&str, usize, usize)] = &[
    // Vowel positions
    ("a", 1, 0), ("e", 1, 7), ("i", 1, 8), ("o", 1, 9), ("u", 0, 7),
    // Consonant positions
    ("k", 2, 6), ("s", 1, 2), ("t", 1, 3), ("n", 1, 6), ("h", 1, 5),
    ("m", 2, 7), ("y", 0, 8), ("r", 1, 1), ("w", 0, 1), ("g", 0, 4),
    ("z", 2, 0), ("d", 1, 4), ("b", 2, 4), ("p", 0, 3), ("f", 0, 2),
    ("j", 0, 5), ("l", 0, 6), ("v", 2, 3), ("q", 0, 0), ("x", 2, 1),
    ("c", 2, 2),
];

/// かな文字からローマ字音素への分解マップ
fn romaji_phonemes(c: char) -> (Option<&'static str>, Option<&'static str>) {
    // (子音, 母音) を返す。母音のみの場合は (None, 母音)
    match c {
        'あ' => (None, Some("a")),
        'い' => (None, Some("i")),
        'う' => (None, Some("u")),
        'え' => (None, Some("e")),
        'お' => (None, Some("o")),
        'か' => (Some("k"), Some("a")),
        'き' => (Some("k"), Some("i")),
        'く' => (Some("k"), Some("u")),
        'け' => (Some("k"), Some("e")),
        'こ' => (Some("k"), Some("o")),
        'さ' => (Some("s"), Some("a")),
        'し' => (Some("s"), Some("i")),
        'す' => (Some("s"), Some("u")),
        'せ' => (Some("s"), Some("e")),
        'そ' => (Some("s"), Some("o")),
        'た' => (Some("t"), Some("a")),
        'ち' => (Some("t"), Some("i")),
        'つ' => (Some("t"), Some("u")),
        'て' => (Some("t"), Some("e")),
        'と' => (Some("t"), Some("o")),
        'な' => (Some("n"), Some("a")),
        'に' => (Some("n"), Some("i")),
        'ぬ' => (Some("n"), Some("u")),
        'ね' => (Some("n"), Some("e")),
        'の' => (Some("n"), Some("o")),
        'は' => (Some("h"), Some("a")),
        'ひ' => (Some("h"), Some("i")),
        'ふ' => (Some("h"), Some("u")),
        'へ' => (Some("h"), Some("e")),
        'ほ' => (Some("h"), Some("o")),
        'ま' => (Some("m"), Some("a")),
        'み' => (Some("m"), Some("i")),
        'む' => (Some("m"), Some("u")),
        'め' => (Some("m"), Some("e")),
        'も' => (Some("m"), Some("o")),
        'や' => (Some("y"), Some("a")),
        'ゆ' => (Some("y"), Some("u")),
        'よ' => (Some("y"), Some("o")),
        'ら' => (Some("r"), Some("a")),
        'り' => (Some("r"), Some("i")),
        'る' => (Some("r"), Some("u")),
        'れ' => (Some("r"), Some("e")),
        'ろ' => (Some("r"), Some("o")),
        'わ' => (Some("w"), Some("a")),
        'を' => (Some("w"), Some("o")),
        'ん' => (Some("n"), None),
        'が' => (Some("g"), Some("a")),
        'ぎ' => (Some("g"), Some("i")),
        'ぐ' => (Some("g"), Some("u")),
        'げ' => (Some("g"), Some("e")),
        'ご' => (Some("g"), Some("o")),
        'ざ' => (Some("z"), Some("a")),
        'じ' => (Some("z"), Some("i")),
        'ず' => (Some("z"), Some("u")),
        'ぜ' => (Some("z"), Some("e")),
        'ぞ' => (Some("z"), Some("o")),
        'だ' => (Some("d"), Some("a")),
        'ぢ' => (Some("d"), Some("i")),
        'づ' => (Some("d"), Some("u")),
        'で' => (Some("d"), Some("e")),
        'ど' => (Some("d"), Some("o")),
        'ば' => (Some("b"), Some("a")),
        'び' => (Some("b"), Some("i")),
        'ぶ' => (Some("b"), Some("u")),
        'べ' => (Some("b"), Some("e")),
        'ぼ' => (Some("b"), Some("o")),
        'ぱ' => (Some("p"), Some("a")),
        'ぴ' => (Some("p"), Some("i")),
        'ぷ' => (Some("p"), Some("u")),
        'ぺ' => (Some("p"), Some("e")),
        'ぽ' => (Some("p"), Some("o")),
        _ => (None, None),
    }
}

// ============================================================================
// Data Structures
// ============================================================================

/// Represents a keyboard layout with 3 layers
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout {
    /// Layer 0: 無シフト, Layer 1: ☆シフト, Layer 2: ★シフト
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
    /// Shift balance: balance between Layer1 (☆) and Layer2 (★) - 100=perfect balance, 0=all on one layer
    pub shift_balance: f64,
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
#[derive(Debug, Clone)]
pub struct CorpusStats {
    pub char_freq: HashMap<char, usize>,
    pub bigram_freq: HashMap<(char, char), usize>,
    pub trigram_freq: HashMap<(char, char, char), usize>,
    pub fourgram_freq: HashMap<(char, char, char, char), usize>,
    pub total_chars: usize,
}

impl CorpusStats {
    /// Load from raw corpus text (legacy method)
    pub fn from_text(text: &str) -> Self {
        let mut char_freq: HashMap<char, usize> = HashMap::new();
        let mut bigram_freq: HashMap<(char, char), usize> = HashMap::new();
        let mut trigram_freq: HashMap<(char, char, char), usize> = HashMap::new();
        let mut fourgram_freq: HashMap<(char, char, char, char), usize> = HashMap::new();
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
        
        for window in chars.windows(3) {
            let trigram = (window[0], window[1], window[2]);
            *trigram_freq.entry(trigram).or_insert(0) += 1;
        }
        
        for window in chars.windows(4) {
            let fourgram = (window[0], window[1], window[2], window[3]);
            *fourgram_freq.entry(fourgram).or_insert(0) += 1;
        }
        
        Self { char_freq, bigram_freq, trigram_freq, fourgram_freq, total_chars }
    }
    
    /// Load from pre-computed n-gram files (more accurate)
    /// Format: count\tcharacters\tn
    /// 〓 is filtered out as it represents line breaks
    pub fn from_ngram_files(
        gram1_path: &str,
        gram2_path: &str,
        gram3_path: &str,
        gram4_path: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut char_freq: HashMap<char, usize> = HashMap::new();
        let mut bigram_freq: HashMap<(char, char), usize> = HashMap::new();
        let mut trigram_freq: HashMap<(char, char, char), usize> = HashMap::new();
        let mut fourgram_freq: HashMap<(char, char, char, char), usize> = HashMap::new();
        let mut total_chars = 0;
        
        // Load 1-gram
        let content = fs::read_to_string(gram1_path)?;
        for line in content.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let count: usize = parts[0].trim().parse().unwrap_or(0);
                let chars: Vec<char> = parts[1].chars().collect();
                if chars.len() == 1 && chars[0] != '〓' {
                    char_freq.insert(chars[0], count);
                    total_chars += count;
                }
            }
        }
        
        // Load 2-gram
        let content = fs::read_to_string(gram2_path)?;
        for line in content.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let count: usize = parts[0].trim().parse().unwrap_or(0);
                let chars: Vec<char> = parts[1].chars().collect();
                if chars.len() == 2 && !chars.contains(&'〓') {
                    bigram_freq.insert((chars[0], chars[1]), count);
                }
            }
        }
        
        // Load 3-gram
        let content = fs::read_to_string(gram3_path)?;
        for line in content.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let count: usize = parts[0].trim().parse().unwrap_or(0);
                let chars: Vec<char> = parts[1].chars().collect();
                if chars.len() == 3 && !chars.contains(&'〓') {
                    trigram_freq.insert((chars[0], chars[1], chars[2]), count);
                }
            }
        }
        
        // Load 4-gram
        let content = fs::read_to_string(gram4_path)?;
        for line in content.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let count: usize = parts[0].trim().parse().unwrap_or(0);
                let chars: Vec<char> = parts[1].chars().collect();
                if chars.len() == 4 && !chars.contains(&'〓') {
                    fourgram_freq.insert((chars[0], chars[1], chars[2], chars[3]), count);
                }
            }
        }
        
        println!("Loaded n-grams: 1g={}, 2g={}, 3g={}, 4g={}, total_chars={}",
            char_freq.len(), bigram_freq.len(), trigram_freq.len(), fourgram_freq.len(), total_chars);
        
        Ok(Self { char_freq, bigram_freq, trigram_freq, fourgram_freq, total_chars })
    }
}

/// Tsuki (月) layout reference for similarity comparison
#[derive(Clone)]
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
        // Layer 0 (無シフト)
        layers[0][1][2] = '☆';  // d key: ☆シフトキー
        layers[0][1][7] = '★';  // k key: ★シフトキー
        layers[0][2][7] = '、';
        layers[0][2][8] = '。';
        // Layer 1 (☆シフト)
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
        
        // Fixed positions on Layer 0 (無シフト):
        layers[0][1][2] = '☆';  // d key: ☆シフトキー
        layers[0][1][7] = '★';  // k key: ★シフトキー
        layers[0][2][7] = '、';
        layers[0][2][8] = '。';
        
        // Fixed positions on Layer 1 (☆シフト):
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
        // Layer 0 (無シフト)
        child.layers[0][1][2] = '☆';  // d key: ☆シフトキー
        child.layers[0][1][7] = '★';  // k key: ★シフトキー
        child.layers[0][2][7] = '、';
        child.layers[0][2][8] = '。';
        // Layer 1 (☆シフト)
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
    /// Layer 0 (無シフト):
    /// - row 1, col 2 (d key): ☆シフトキー
    /// - row 1, col 7 (k key): ★シフトキー
    /// - row 2, col 7: 、
    /// - row 2, col 8: 。
    /// Layer 1 (☆シフト):
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
        // Layer 0 (無シフト)
        self.layers[0][1][2] = '☆';  // d key: ☆シフトキー
        self.layers[0][1][7] = '★';  // k key: ★シフトキー
        self.layers[0][2][7] = '、';
        self.layers[0][2][8] = '。';
        // Layer 1 (☆シフト)
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

#[derive(Clone)]
pub struct Evaluator {
    pub corpus: CorpusStats,
    pub tsuki: TsukiLayout,
    pub weights: EvaluationWeights,
}

impl Evaluator {
    pub fn new(corpus_text: &str) -> Self {
        Self {
            corpus: CorpusStats::from_text(corpus_text),
            tsuki: TsukiLayout::new(),
            weights: EvaluationWeights::default(),
        }
    }
    
    pub fn from_corpus(corpus: CorpusStats) -> Self {
        Self {
            corpus,
            tsuki: TsukiLayout::new(),
            weights: EvaluationWeights::default(),
        }
    }
    
    pub fn from_corpus_with_weights(corpus: CorpusStats, weights: EvaluationWeights) -> Self {
        Self {
            corpus,
            tsuki: TsukiLayout::new(),
            weights,
        }
    }
    
    /// Evaluate a layout and return fitness score
    /// Uses hybrid multiplicative-additive approach:
    /// fitness = core_multiplier × additive_bonus
    pub fn evaluate(&self, layout: &mut Layout) -> f64 {
        let scores = self.compute_scores(layout);
        let w = &self.weights;
        
        // ============================================================
        // Core Metrics (乗算・幾何平均): 低スコアが全体を大きく下げる
        // ============================================================
        let same_finger_norm = (scores.same_finger / 100.0).max(0.01);
        let row_skip_norm = (scores.row_skip / 100.0).max(0.01);
        let home_norm = (scores.home_position / 100.0).max(0.01);
        let total_keystrokes_norm = (scores.total_keystrokes / 100.0).max(0.01);
        let alternating_norm = (scores.alternating / 100.0).max(0.01);
        let single_key_norm = (scores.single_key / 100.0).max(0.01);
        let colemak_norm = (scores.colemak_similarity / 100.0).max(0.01);
        
        // Weighted geometric mean: (a^w1 * b^w2 * ...)^(1/sum_w)
        let total_weight = w.same_finger + w.row_skip + w.home_position
            + w.total_keystrokes + w.alternating + w.single_key
            + w.colemak_similarity;
        let core_product = 
            same_finger_norm.powf(w.same_finger) *
            row_skip_norm.powf(w.row_skip) *
            home_norm.powf(w.home_position) *
            total_keystrokes_norm.powf(w.total_keystrokes) *
            alternating_norm.powf(w.alternating) *
            single_key_norm.powf(w.single_key) *
            colemak_norm.powf(w.colemak_similarity);
        let core_multiplier = core_product.powf(1.0 / total_weight) * 100.0;
        
        // ============================================================
        // Bonus Metrics (加算): 高スコアでボーナス、低くてもペナルティ小
        // ============================================================
        let additive_bonus = 
            scores.redirect_low * w.redirect_low +
            scores.tsuki_similarity * w.tsuki_similarity +
            scores.roll * w.roll +
            scores.inroll * w.inroll +
            scores.arpeggio * w.arpeggio +
            scores.memorability * w.memorability +
            scores.shift_balance * w.shift_balance;
        
        // Final fitness: core × (1 + bonus/scale)
        let bonus_scale = w.redirect_low + w.tsuki_similarity + w.roll 
            + w.inroll + w.arpeggio + w.memorability + w.shift_balance;
        let bonus_scale = bonus_scale * 100.0; // Scale to match typical bonus values
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
        let mut layer1_keystrokes = 0.0;  // ☆シフト
        let mut layer2_keystrokes = 0.0;  // ★シフト
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
                
                // Shifted characters - track each layer separately for balance
                if pos.layer > 0 {
                    shifted_keystrokes += count_f;
                    if pos.layer == 1 {
                        layer1_keystrokes += count_f;  // ☆シフト
                    } else if pos.layer == 2 {
                        layer2_keystrokes += count_f;  // ★シフト
                    }
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
            
            // シフトバランス (前置シフト用: Layer1とLayer2の均等化)
            // 100% = 完全均等 (50:50), 0% = 片方のみ使用
            shift_balance: {
                let total_shifted = layer1_keystrokes + layer2_keystrokes;
                if total_shifted > 0.0 {
                    let ratio = layer1_keystrokes.min(layer2_keystrokes) 
                              / layer1_keystrokes.max(layer2_keystrokes);
                    100.0 * ratio  // 1.0 = perfect balance, 0.0 = all on one side
                } else {
                    100.0  // No shift needed = perfect
                }
            },
        }
    }
    
    /// Colemak類似度計算
    /// - Layer 0: 100%の重み
    /// - Layer 1, 2: 80%の重み
    fn calc_colemak_similarity(&self, layout: &Layout) -> f64 {
        let mut total_score = 0.0;
        let mut max_score = 0.0;
        
        // Build position map: phoneme -> (row, col)
        let mut phoneme_pos: HashMap<&str, (usize, usize)> = HashMap::new();
        for &(phoneme, row, col) in COLEMAK_POSITIONS {
            phoneme_pos.insert(phoneme, (row, col));
        }
        
        // Check all layers with different weights
        // Layer 0: 100%, Layer 1/2: 80%
        for layer in 0..NUM_LAYERS {
            let layer_weight = if layer == 0 { 1.0 } else { 0.8 };
            
            for row in 0..ROWS {
                for col in 0..COLS {
                    let c = layout.layers[layer][row][col];
                    if c == '☆' || c == '★' || c == '、' || c == '。' || c == '　' || c == '\0' || c == '゛' || c == '゜' {
                        continue;
                    }
                    
                    let (consonant, vowel) = romaji_phonemes(c);
                    
                    // Calculate max possible score for this character (weighted by layer)
                    let char_max_score = match (consonant, vowel) {
                        (Some(_), Some(_)) => 2.0 * layer_weight,
                        (Some(_), None) | (None, Some(_)) => 1.0 * layer_weight,
                        (None, None) => 0.0,
                    };
                    max_score += char_max_score;
                    
                    if char_max_score == 0.0 {
                        continue;
                    }
                    
                    // Check consonant match
                    if let Some(cons) = consonant {
                        if let Some(&(exp_row, exp_col)) = phoneme_pos.get(cons) {
                            if row == exp_row && col == exp_col {
                                total_score += 1.0 * layer_weight; // Perfect consonant match
                            } else if row == exp_row {
                                total_score += 0.5 * layer_weight; // Same row
                            } else if (col < 5 && exp_col < 5) || (col >= 5 && exp_col >= 5) {
                                total_score += 0.25 * layer_weight; // Same hand
                            }
                        }
                    }
                    
                    // Check vowel match
                    if let Some(vow) = vowel {
                        if let Some(&(exp_row, exp_col)) = phoneme_pos.get(vow) {
                            if row == exp_row && col == exp_col {
                                total_score += 1.0 * layer_weight; // Perfect vowel match
                            } else if row == exp_row {
                                total_score += 0.5 * layer_weight; // Same row
                            } else if (col < 5 && exp_col < 5) || (col >= 5 && exp_col >= 5) {
                                total_score += 0.25 * layer_weight; // Same hand
                            }
                        }
                    }
                }
            }
        }
        
        if max_score > 0.0 {
            100.0 * total_score / max_score
        } else {
            0.0
        }
    }
    
    /// 月配列類似度計算
    /// - Layer 0: 計算しない（除外）
    /// - Layer 1, 2のみ: 100%で計算
    fn calc_tsuki_similarity(&self, layout: &Layout) -> f64 {
        let mut matches = 0;
        let mut total = 0;
        
        // Only check Layer 1 and Layer 2 (not Layer 0)
        for layer in 1..NUM_LAYERS {
            for row in 0..ROWS {
                for col in 0..COLS {
                    let c = layout.layers[layer][row][col];
                    if c == '☆' || c == '★' || c == '、' || c == '。' || c == '　' || c == '\0' || c == '゛' || c == '゜' {
                        continue;
                    }
                    
                    // Check if this character exists in tsuki layout at same position
                    if let Some(&tsuki_pos) = self.tsuki.char_positions.get(&c) {
                        total += 1;
                        // Position match (row, col) - layer doesn't need to match since tsuki is 2-layer
                        if row == tsuki_pos.row && col == tsuki_pos.col {
                            matches += 1;
                        }
                    }
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
    
    /// Calculate shift balance between Layer 1 (☆) and Layer 2 (★)
    /// For pre-shift (前置シフト), both layers require same number of keystrokes,
    /// so the usage frequency should be balanced for ergonomics.
    /// Returns 100% for perfect balance (50:50), 0% if all on one layer.
    pub fn calc_shift_balance(&self, layout: &Layout) -> f64 {
        let mut layer1_keystrokes = 0.0;  // ☆シフト
        let mut layer2_keystrokes = 0.0;  // ★シフト
        
        for (&c, &count) in &self.corpus.char_freq {
            if let Some(pos) = layout.find_char(c) {
                let count_f = count as f64;
                if pos.layer == 1 {
                    layer1_keystrokes += count_f;
                } else if pos.layer == 2 {
                    layer2_keystrokes += count_f;
                }
            }
        }
        
        let total_shifted = layer1_keystrokes + layer2_keystrokes;
        if total_shifted > 0.0 {
            let ratio = layer1_keystrokes.min(layer2_keystrokes) 
                      / layer1_keystrokes.max(layer2_keystrokes);
            100.0 * ratio  // 1.0 = perfect balance (50:50)
        } else {
            100.0  // No shift needed = perfect
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
        Self::from_corpus(population_size, CorpusStats::from_text(corpus_text), seed)
    }
    
    pub fn from_corpus(
        population_size: usize,
        corpus: CorpusStats,
        seed: u64,
    ) -> Self {
        Self::from_corpus_with_weights(population_size, corpus, seed, EvaluationWeights::default())
    }
    
    pub fn from_corpus_with_weights(
        population_size: usize,
        corpus: CorpusStats,
        seed: u64,
        weights: EvaluationWeights,
    ) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let evaluator = Evaluator::from_corpus_with_weights(corpus, weights);
        
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
        // Parallel evaluation - clone evaluator for each thread
        let evaluator = self.evaluator.clone();
        self.population.par_iter_mut().for_each(|layout| {
            evaluator.evaluate(layout);
        });
        
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
        Self::from_corpus(population_size, CorpusStats::from_text(corpus_text), seed, use_gpu)
    }
    
    pub fn from_corpus(
        population_size: usize,
        corpus: CorpusStats,
        seed: u64,
        use_gpu: bool,
    ) -> Self {
        Self::from_corpus_with_weights(population_size, corpus, seed, use_gpu, EvaluationWeights::default())
    }
    
    pub fn from_corpus_with_weights(
        population_size: usize,
        corpus: CorpusStats,
        seed: u64,
        use_gpu: bool,
        weights: EvaluationWeights,
    ) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let evaluator = Evaluator::from_corpus_with_weights(corpus, weights);
        
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
                layout.fitness = compute_weighted_fitness(&scores, &self.evaluator.weights);
            }
        } else {
            // CPU evaluation (parallel)
            let evaluator = self.evaluator.clone();
            self.population.par_iter_mut().for_each(|layout| {
                evaluator.evaluate(layout);
            });
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
        
        // Shift balance requires CPU calculation (GPU doesn't track layer1/layer2 separately)
        shift_balance: evaluator.calc_shift_balance(layout),
    }
}

/// Compute weighted fitness from scores using hybrid multiplicative-additive approach
fn compute_weighted_fitness(scores: &EvaluationScores, weights: &EvaluationWeights) -> f64 {
    // Core metrics
    let same_finger_norm = (scores.same_finger / 100.0).max(0.01);
    let row_skip_norm = (scores.row_skip / 100.0).max(0.01);
    let home_norm = (scores.home_position / 100.0).max(0.01);
    let total_keystrokes_norm = (scores.total_keystrokes / 100.0).max(0.01);
    let alternating_norm = (scores.alternating / 100.0).max(0.01);
    let single_key_norm = (scores.single_key / 100.0).max(0.01);
    let colemak_norm = (scores.colemak_similarity / 100.0).max(0.01);
    
    let total_weight = weights.same_finger + weights.row_skip + weights.home_position
        + weights.total_keystrokes + weights.alternating + weights.single_key
        + weights.colemak_similarity;
    let core_product = 
        same_finger_norm.powf(weights.same_finger) *
        row_skip_norm.powf(weights.row_skip) *
        home_norm.powf(weights.home_position) *
        total_keystrokes_norm.powf(weights.total_keystrokes) *
        alternating_norm.powf(weights.alternating) *
        single_key_norm.powf(weights.single_key) *
        colemak_norm.powf(weights.colemak_similarity);
    let core_multiplier = core_product.powf(1.0 / total_weight) * 100.0;
    
    // Bonus metrics
    let additive_bonus = 
        scores.redirect_low * weights.redirect_low +
        scores.tsuki_similarity * weights.tsuki_similarity +
        scores.roll * weights.roll +
        scores.inroll * weights.inroll +
        scores.arpeggio * weights.arpeggio +
        scores.memorability * weights.memorability +
        scores.shift_balance * weights.shift_balance;
    
    let bonus_scale = weights.redirect_low + weights.tsuki_similarity + weights.roll 
        + weights.inroll + weights.arpeggio + weights.memorability + weights.shift_balance;
    let bonus_scale = bonus_scale * 100.0;
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
            0 => "無シフト",
            1 => "☆シフト",
            2 => "★シフト",
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
    
    let num_cores = num_cpus::get();
    let num_runs = if args.multi_run > 0 { 
        args.multi_run.min(num_cores) // Limit to available cores
    } else { 
        1 
    };
    
    // Set thread count for rayon
    // In multi-run mode, maximize parallelism for maximum performance
    if args.multi_run > 0 {
        // Use all available cores for multi-run mode
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_cores)
            .build_global()
            .unwrap();
    } else if args.threads > 0 {
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
    println!("System Info:");
    println!("  CPU Cores: {}", num_cores);
    if num_runs > 1 {
        println!("  Parallel Runs: {} (max {} cores)", num_runs, num_cores);
        if args.multi_run > num_cores {
            println!("  Warning: Requested {} runs but limited to {} cores", args.multi_run, num_cores);
        }
    }
    println!();
    println!("Configuration:");
    println!("  Population: {}", args.population);
    println!("  Generations: {}", args.generations);
    println!("  Mutation Rate: {:.2}", args.mutation_rate);
    println!("  Elite Count: {}", args.elite);
    println!("  Seed: {} (base seed)", args.seed);
    println!("  GPU Acceleration: {}", if args.no_gpu { "Disabled (CPU mode)" } else { "Auto (hybrid mode)" });
    println!();
    
    // Load corpus from n-gram files (preferred) or raw corpus text (fallback)
    let corpus = if args.gram1.is_some() && args.gram2.is_some() && args.gram3.is_some() && args.gram4.is_some() {
        println!("Loading from n-gram files (〓 filtered out)...");
        CorpusStats::from_ngram_files(
            args.gram1.as_ref().unwrap().to_str().unwrap(),
            args.gram2.as_ref().unwrap().to_str().unwrap(),
            args.gram3.as_ref().unwrap().to_str().unwrap(),
            args.gram4.as_ref().unwrap().to_str().unwrap(),
        ).expect("Failed to load n-gram files")
    } else {
        println!("Loading from corpus text file...");
        let corpus_text = fs::read_to_string(&args.corpus)
            .expect("Failed to read corpus file");
        
        let hiragana_count: usize = corpus_text.chars()
            .filter(|c| is_hiragana(*c) || *c == '、' || *c == '。')
            .count();
        println!("Loaded corpus: {} bytes, {} hiragana characters", corpus_text.len(), hiragana_count);
        CorpusStats::from_text(&corpus_text)
    };
    
    if num_runs > 1 {
        // Multi-run mode: parallel execution with different seeds
        run_multi_ga(&args, corpus, num_runs);
    } else {
        // Single run mode (original behavior with TUI support)
        let (tui_state, tui_handle) = if args.tui {
            let state = Arc::new(Mutex::new(tui::TuiState::new(args.generations)));
            let state_clone = Arc::clone(&state);
            
            let handle = thread::spawn(move || {
                if let Err(e) = tui::run_tui(state_clone) {
                    eprintln!("TUI error: {}", e);
                }
            });
            
            thread::sleep(std::time::Duration::from_millis(100));
            (Some(state), Some(handle))
        } else {
            (None, None)
        };
        
        if !args.no_gpu {
            run_gpu_ga(&args, corpus, tui_state, tui_handle);
        } else {
            run_cpu_ga(&args, corpus, tui_state, tui_handle);
        }
    }
}

/// Multi-run GA: Execute multiple GA runs in parallel with different seeds
fn run_multi_ga(args: &Args, corpus: CorpusStats, num_runs: usize) {
    use rayon::prelude::*;
    use std::time::Instant;
    use rand::Rng;
    use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
    
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║               MULTI-RUN MODE: {} PARALLEL RUNS                 ║", num_runs);
    println!("╚══════════════════════════════════════════════════════════════╝\n");
    
    let start_time = Instant::now();
    let weights = EvaluationWeights::from(args);
    
    // Generate random seeds for each run
    let mut rng = rand::thread_rng();
    let seeds: Vec<u64> = (0..num_runs)
        .map(|_| rng.gen::<u64>())
        .collect();
    
    println!("Random Seeds Generated (truncated): [{}, {}, ..., {}]", 
        seeds.first().unwrap(), 
        seeds.get(1).unwrap_or(&0), 
        seeds.last().unwrap()
    );
    println!("Starting parallel execution on {} cores...\n", num_cpus::get().min(num_runs));
    
    // Initialize TUI state for multi-run mode (only if TTY available)
    let tui_state = Arc::new(Mutex::new(tui::TuiState::new(args.generations)));
    let tui_state_clone = Arc::clone(&tui_state);
    
    // Try to start TUI in separate thread (may fail in non-TTY environments)
    let tui_handle = if atty::is(atty::Stream::Stdout) {
        println!("TUI mode enabled for multi-run (press 'q' to quit after completion)\n");
        Some(thread::spawn(move || {
            if let Err(e) = tui::run_tui(tui_state_clone) {
                eprintln!("TUI error: {}", e);
            }
        }))
    } else {
        println!("TUI mode disabled (no TTY detected)\n");
        None
    };
    
    // Give TUI time to initialize
    if tui_handle.is_some() {
        thread::sleep(std::time::Duration::from_millis(100));
    }
    
    // Create multi-progress for parallel progress tracking
    let multi_progress = Arc::new(MultiProgress::new());
    
    // Parallel execution of multiple GA runs with TUI state sharing
    let results: Vec<(u64, Layout, f64)> = seeds.par_iter().enumerate().map(|(idx, &seed)| {
        let tui_state_ref = Arc::clone(&tui_state);
        
        // Clone corpus for each thread
        let corpus_clone = corpus.clone();
        let weights_clone = weights.clone();
        
        // Create modified args for this run
        let mut run_args = args.clone();
        run_args.seed = seed;
        run_args.tui = false; // Disable TUI in multi-run mode
        
        // Run GA
        let result = if args.no_gpu {
            let mut ga = GeneticAlgorithm::from_corpus_with_weights(
                args.population,
                corpus_clone,
                seed,
                weights_clone
            );
            
            // Evaluate initial population
            for layout in &mut ga.population {
                ga.evaluator.evaluate(layout);
            }
            ga.population.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
            ga.best_fitness = ga.population[0].fitness;
            ga.best_layout = ga.population[0].clone();
            
            // Update TUI with initial best
            {
                let mut state = tui_state_ref.lock().unwrap();
                if ga.best_fitness > state.best_fitness {
                    state.update(0, ga.best_fitness, &ga.best_layout, &ga.best_layout.scores);
                }
            }
            
            // Evolution with TUI state updates
            for gen in 0..args.generations {
                ga.evolve(args.mutation_rate, args.elite);
                
                // Update TUI if this is the new global best
                let mut state = tui_state_ref.lock().unwrap();
                if ga.best_fitness > state.best_fitness {
                    state.update(gen + 1, ga.best_fitness, &ga.best_layout, &ga.best_layout.scores);
                }
            }
            
            (seed, ga.best_layout, ga.best_fitness)
        } else {
            let mut ga = GpuGeneticAlgorithm::from_corpus_with_weights(
                args.population,
                corpus_clone,
                seed,
                true,
                weights_clone
            );
            
            ga.evaluate_all();
            ga.population.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
            ga.best_fitness = ga.population[0].fitness;
            ga.best_layout = ga.population[0].clone();
            
            // Update TUI with initial best
            {
                let mut state = tui_state_ref.lock().unwrap();
                if ga.best_fitness > state.best_fitness {
                    state.update(0, ga.best_fitness, &ga.best_layout, &ga.best_layout.scores);
                }
            }
            
            // Evolution with TUI state updates
            for gen in 0..args.generations {
                ga.evolve(args.mutation_rate, args.elite);
                
                // Update TUI if this is the new global best
                let mut state = tui_state_ref.lock().unwrap();
                if ga.best_fitness > state.best_fitness {
                    state.update(gen + 1, ga.best_fitness, &ga.best_layout, &ga.best_layout.scores);
                }
            }
            
            (seed, ga.best_layout, ga.best_fitness)
        };
        
        result
    }).collect();
    
    let elapsed = start_time.elapsed();
    
    // Wait for user to quit TUI (if it was started)
    if let Some(handle) = tui_handle {
        if let Err(e) = handle.join() {
            eprintln!("TUI thread error: {:?}", e);
        }
    }
    
    // Print summary
    print_multi_run_summary(&results, args, &weights, elapsed);
}

fn print_multi_run_summary(
    results: &[(u64, Layout, f64)],
    args: &Args,
    weights: &EvaluationWeights,
    elapsed: std::time::Duration
) {
    println!("\n\n╔══════════════════════════════════════════════════════════════╗");
    println!("║              MULTI-RUN RESULTS SUMMARY                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    
    println!("\nTotal Time: {:.2}s ({:.2}s per run average)", 
        elapsed.as_secs_f64(), 
        elapsed.as_secs_f64() / results.len() as f64
    );
    
    // Find best and worst
    let best = results.iter().max_by(|a, b| a.2.partial_cmp(&b.2).unwrap()).unwrap();
    let worst = results.iter().min_by(|a, b| a.2.partial_cmp(&b.2).unwrap()).unwrap();
    
    // Calculate statistics
    let fitnesses: Vec<f64> = results.iter().map(|(_, _, f)| *f).collect();
    let mean = fitnesses.iter().sum::<f64>() / fitnesses.len() as f64;
    let variance = fitnesses.iter()
        .map(|f| (f - mean).powi(2))
        .sum::<f64>() / fitnesses.len() as f64;
    let std_dev = variance.sqrt();
    
    println!("\nStatistics:");
    println!("  Runs: {}", results.len());
    println!("  Best:  {:.4} (seed: {})", best.2, best.0);
    println!("  Worst: {:.4} (seed: {})", worst.2, worst.0);
    println!("  Mean:  {:.4}", mean);
    println!("  StdDev: {:.4}", std_dev);
    println!("  Range: {:.4}", best.2 - worst.2);
    
    println!("\nAll Results:");
    println!("┌──────────┬──────────┐");
    println!("│   Seed   │ Fitness  │");
    println!("├──────────┼──────────┤");
    for (seed, _, fitness) in results.iter() {
        let marker = if *seed == best.0 { " ★" } else if *seed == worst.0 { " ☆" } else { "  " };
        println!("│ {:8} │ {:7.4}{} │", seed, fitness, marker);
    }
    println!("└──────────┴──────────┘");
    
    // Print best layout details
    println!("\n\n=== BEST LAYOUT (Seed: {}) ===", best.0);
    println!("{}", format_layout(&best.1));
    
    // Save best layout
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let best_filename = format!("ga_layout_best_seed{}_{}runs_{}.json", best.0, results.len(), timestamp);
    
    // Create modified args with best seed for filename
    let mut best_args = args.clone();
    best_args.seed = best.0;
    let analyzer_json = generate_keyboard_analyzer_json(&best.1, &best_args);
    fs::write(&best_filename, serde_json::to_string_pretty(&analyzer_json).unwrap()).unwrap();
    println!("\nBest layout saved to: {}", best_filename);
    
    // Optionally print detailed scores for best
    print_final_results(&best.1, best.2, args, weights);
}

fn run_cpu_ga(
    args: &Args, 
    corpus: CorpusStats, 
    tui_state: Option<Arc<Mutex<tui::TuiState>>>,
    tui_handle: Option<thread::JoinHandle<()>>
) {
    // Get weights from args
    let weights = EvaluationWeights::from(args);
    
    // Initialize GA
    let mut ga = GeneticAlgorithm::from_corpus_with_weights(args.population, corpus, args.seed, weights);
    
    // Evaluate initial population
    for layout in &mut ga.population {
        ga.evaluator.evaluate(layout);
    }
    ga.population.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
    ga.best_fitness = ga.population[0].fitness;
    ga.best_layout = ga.population[0].clone();
    
    // Initialize TUI with generation 0
    if let Some(ref state) = tui_state {
        let mut state = state.lock().unwrap();
        state.update(0, ga.best_fitness, &ga.best_layout, &ga.best_layout.scores);
    }
    
    if !args.tui {
        println!("Initial best fitness: {:.4}", ga.best_fitness);
    }
    
    // Progress bar (only if not in TUI mode)
    let pb = if !args.tui {
        let pb = ProgressBar::new(args.generations as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} (ETA: {eta}) | Best: {msg}")
                .unwrap()
                .progress_chars("#>-")
        );
        Some(pb)
    } else {
        None
    };
    
    // Evolution loop
    for gen in 0..args.generations {
        ga.evolve(args.mutation_rate, args.elite);
        
        if let Some(ref pb) = pb {
            pb.set_position((gen + 1) as u64);
            pb.set_message(format!("{:.4}", ga.best_fitness));
        }
        
        // Update TUI state
        if let Some(ref state) = tui_state {
            let mut state = state.lock().unwrap();
            state.update(gen + 1, ga.best_fitness, &ga.best_layout, &ga.best_layout.scores);
        }
        
        // Print progress every 100 generations (only if not in TUI mode)
        if !args.tui && (gen + 1) % 100 == 0 {
            print_progress(gen + 1, &ga.best_layout, ga.best_fitness, &ga.evaluator.weights);
        }
    }
    
    if let Some(pb) = pb {
        pb.finish_with_message(format!("{:.4}", ga.best_fitness));
    }
    
    if !args.tui {
        print_final_results(&ga.best_layout, ga.best_fitness, args, &ga.evaluator.weights);
    } else {
        // Wait for user to quit TUI
        if let Some(handle) = tui_handle {
            let _ = handle.join();
        }
        print_final_results(&ga.best_layout, ga.best_fitness, args, &ga.evaluator.weights);
    }
}

fn run_gpu_ga(
    args: &Args, 
    corpus: CorpusStats, 
    tui_state: Option<Arc<Mutex<tui::TuiState>>>,
    tui_handle: Option<thread::JoinHandle<()>>
) {
    // Get weights from args
    let weights = EvaluationWeights::from(args);
    
    // Initialize GPU-accelerated GA
    let mut ga = GpuGeneticAlgorithm::from_corpus_with_weights(args.population, corpus, args.seed, true, weights);
    
    // Evaluate initial population
    ga.evaluate_all();
    ga.population.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
    ga.best_fitness = ga.population[0].fitness;
    ga.best_layout = ga.population[0].clone();
    
    // Initialize TUI with generation 0
    if let Some(ref state) = tui_state {
        let mut state = state.lock().unwrap();
        state.update(0, ga.best_fitness, &ga.best_layout, &ga.best_layout.scores);
    }
    
    if !args.tui {
        println!("Initial best fitness: {:.4}", ga.best_fitness);
    }
    
    // Progress bar (only if not in TUI mode)
    let pb = if !args.tui {
        let pb = ProgressBar::new(args.generations as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} (ETA: {eta}) | Best: {msg}")
                .unwrap()
                .progress_chars("#>-")
        );
        Some(pb)
    } else {
        None
    };
    
    // Evolution loop
    for gen in 0..args.generations {
        ga.evolve(args.mutation_rate, args.elite);
        
        if let Some(ref pb) = pb {
            pb.set_position((gen + 1) as u64);
            pb.set_message(format!("{:.4}", ga.best_fitness));
        }
        
        // Update TUI state
        if let Some(ref state) = tui_state {
            let mut state = state.lock().unwrap();
            state.update(gen + 1, ga.best_fitness, &ga.best_layout, &ga.best_layout.scores);
        }
        
        // Print progress every 100 generations (only if not in TUI mode)
        if !args.tui && (gen + 1) % 100 == 0 {
            print_progress(gen + 1, &ga.best_layout, ga.best_fitness, &ga.evaluator.weights);
        }
    }
    
    if let Some(pb) = pb {
        pb.finish_with_message(format!("{:.4}", ga.best_fitness));
    }
    
    if !args.tui {
        print_final_results(&ga.best_layout, ga.best_fitness, args, &ga.evaluator.weights);
    } else {
        // Wait for user to quit TUI
        if let Some(handle) = tui_handle {
            let _ = handle.join();
        }
        print_final_results(&ga.best_layout, ga.best_fitness, args, &ga.evaluator.weights);
    }
}

fn print_progress(gen: usize, layout: &Layout, fitness: f64, weights: &EvaluationWeights) {
    println!("\n\nGeneration {}: Best Fitness = {:.4}", gen, fitness);
    let s = &layout.scores;
    let w = weights;
    
    // Compute core multiplier
    let same_finger_norm = (s.same_finger / 100.0).max(0.01);
    let row_skip_norm = (s.row_skip / 100.0).max(0.01);
    let home_norm = (s.home_position / 100.0).max(0.01);
    let total_keystrokes_norm = (s.total_keystrokes / 100.0).max(0.01);
    let alternating_norm = (s.alternating / 100.0).max(0.01);
    let single_key_norm = (s.single_key / 100.0).max(0.01);
    let colemak_norm = (s.colemak_similarity / 100.0).max(0.01);
    let total_weight = w.same_finger + w.row_skip + w.home_position
        + w.total_keystrokes + w.alternating + w.single_key
        + w.colemak_similarity;
    let core_product = same_finger_norm.powf(w.same_finger)
        * row_skip_norm.powf(w.row_skip)
        * home_norm.powf(w.home_position)
        * total_keystrokes_norm.powf(w.total_keystrokes)
        * alternating_norm.powf(w.alternating)
        * single_key_norm.powf(w.single_key)
        * colemak_norm.powf(w.colemak_similarity);
    let core_multiplier = core_product.powf(1.0 / total_weight) * 100.0;
    
    let additive_bonus = 
        s.redirect_low * w.redirect_low +
        s.tsuki_similarity * w.tsuki_similarity +
        s.roll * w.roll +
        s.inroll * w.inroll +
        s.arpeggio * w.arpeggio +
        s.memorability * w.memorability +
        s.shift_balance * w.shift_balance;
    
    let bonus_scale = w.redirect_low + w.tsuki_similarity + w.roll 
        + w.inroll + w.arpeggio + w.memorability + w.shift_balance;
    let bonus_scale = bonus_scale * 100.0;
    
    println!("\nFitness = Core × (1 + Bonus/{:.0})", bonus_scale);
    println!("        = {:.2} × (1 + {:.2}/{:.0}) = {:.4}", core_multiplier, additive_bonus, bonus_scale, fitness);
    println!("\nCore (乗算・必須): {:.2}", core_multiplier);
    println!("  同指連続低(2g SFB): {:.2}% ^{:.2}", s.same_finger, w.same_finger);
    println!("  段飛ばし少(2g同指): {:.2}% ^{:.2}", s.row_skip, w.row_skip);
    println!("  ホームポジ率:       {:.2}% ^{:.2}", s.home_position, w.home_position);
    println!("  総打鍵コスト少:     {:.2}% ^{:.2}", s.total_keystrokes, w.total_keystrokes);
    println!("  左右交互(2g):       {:.2}% ^{:.2}", s.alternating, w.alternating);
    println!("  単打鍵率:           {:.2}% ^{:.2}", s.single_key, w.single_key);
    println!("  Colemak類似(音素):  {:.2}% ^{:.2}", s.colemak_similarity, w.colemak_similarity);
    
    println!("Bonus (加算・奨励): {:.2}", additive_bonus);
    println!("  リダイレクト少(3g): {:.2} x {:.1} = {:.2}", s.redirect_low, w.redirect_low, s.redirect_low * w.redirect_low);
    println!("  月配列類似:         {:.2} x {:.1} = {:.2}", s.tsuki_similarity, w.tsuki_similarity, s.tsuki_similarity * w.tsuki_similarity);
    println!("  ロール率(3g):       {:.2} x {:.1} = {:.2}", s.roll, w.roll, s.roll * w.roll);
    println!("  インロール(3g):     {:.2} x {:.1} = {:.2}", s.inroll, w.inroll, s.inroll * w.inroll);
    println!("  アルペジオ(3g):     {:.2} x {:.1} = {:.2}", s.arpeggio, w.arpeggio, s.arpeggio * w.arpeggio);
    println!("  覚えやすさ:         {:.2} x {:.1} = {:.2}", s.memorability, w.memorability, s.memorability * w.memorability);
    println!("  シフトバランス:     {:.2} x {:.1} = {:.2}", s.shift_balance, w.shift_balance, s.shift_balance * w.shift_balance);
}

fn print_final_results(layout: &Layout, fitness: f64, args: &Args, weights: &EvaluationWeights) {
    // Print final results
    println!("\n\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    OPTIMIZATION COMPLETE                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("\nFinal Best Fitness: {:.4}", fitness);
    println!("\nBest Layout:");
    println!("{}", format_layout(layout));
    
    let s = &layout.scores;
    let w = weights;
    
    // Compute core multiplier for display
    let same_finger_norm = (s.same_finger / 100.0).max(0.01);
    let row_skip_norm = (s.row_skip / 100.0).max(0.01);
    let home_norm = (s.home_position / 100.0).max(0.01);
    let total_keystrokes_norm = (s.total_keystrokes / 100.0).max(0.01);
    let alternating_norm = (s.alternating / 100.0).max(0.01);
    let single_key_norm = (s.single_key / 100.0).max(0.01);
    let colemak_norm = (s.colemak_similarity / 100.0).max(0.01);
    let total_core_weight = w.same_finger + w.row_skip + w.home_position
        + w.total_keystrokes + w.alternating + w.single_key
        + w.colemak_similarity;
    let core_product = same_finger_norm.powf(w.same_finger)
        * row_skip_norm.powf(w.row_skip)
        * home_norm.powf(w.home_position)
        * total_keystrokes_norm.powf(w.total_keystrokes)
        * alternating_norm.powf(w.alternating)
        * single_key_norm.powf(w.single_key)
        * colemak_norm.powf(w.colemak_similarity);
    let core_multiplier = core_product.powf(1.0 / total_core_weight) * 100.0;
    
    let additive_bonus = 
        s.redirect_low * w.redirect_low +
        s.tsuki_similarity * w.tsuki_similarity +
        s.roll * w.roll +
        s.inroll * w.inroll +
        s.arpeggio * w.arpeggio +
        s.memorability * w.memorability +
        s.shift_balance * w.shift_balance;
    
    let bonus_scale = w.redirect_low + w.tsuki_similarity + w.roll 
        + w.inroll + w.arpeggio + w.memorability + w.shift_balance;
    let bonus_scale = bonus_scale * 100.0;
    
    println!("\n=== Scoring: Core × (1 + Bonus/{:.0}) ===", bonus_scale);
    println!("\nCore Metrics (乗算・必須) - 低スコアは致命的:");
    println!("┌─────────────────────────────┬────────┬────────┐");
    println!("│ Metric                      │ Score  │ Weight │");
    println!("├─────────────────────────────┼────────┼────────┤");
    println!("│ 同指連続低 (2g SFB回避)     │ {:6.2}%│ ^{:.2}  │", s.same_finger, w.same_finger);
    println!("│ 段飛ばし少 (2g同指跳躍)     │ {:6.2}%│ ^{:.2}  │", s.row_skip, w.row_skip);
    println!("│ ホームポジ率 (中段使用)     │ {:6.2}%│ ^{:.2}  │", s.home_position, w.home_position);
    println!("│ 総打鍵コスト少 (距離負担)   │ {:6.2}%│ ^{:.2}  │", s.total_keystrokes, w.total_keystrokes);
    println!("│ 左右交互 (2g手の切替)       │ {:6.2}%│ ^{:.2}  │", s.alternating, w.alternating);
    println!("│ 単打鍵率 (シフト無し)       │ {:6.2}%│ ^{:.2}  │", s.single_key, w.single_key);
    println!("│ Colemak類似 (音素配置)      │ {:6.2}%│ ^{:.2}  │", s.colemak_similarity, w.colemak_similarity);
    println!("├─────────────────────────────┼────────┼────────┤");
    println!("│ Core Multiplier             │ {:6.2} │        │", core_multiplier);
    println!("└─────────────────────────────┴────────┴────────┘");
    
    println!("\nBonus Metrics (加算) - 高スコアで加点:");
    println!("┌─────────────────────────────┬────────┬────────┬──────────┐");
    println!("│ Metric                      │ Score  │ Weight │ Weighted │");
    println!("├─────────────────────────────┼────────┼────────┼──────────┤");
    println!("│ リダイレクト少 (3g方向)     │ {:6.2} │ {:6.1} │ {:8.2} │", s.redirect_low, w.redirect_low, s.redirect_low * w.redirect_low);
    println!("│ 月配列類似 (日本語最適)     │ {:6.2} │ {:6.1} │ {:8.2} │", s.tsuki_similarity, w.tsuki_similarity, s.tsuki_similarity * w.tsuki_similarity);
    println!("│ ロール率 (3g同手の流れ)     │ {:6.2} │ {:6.1} │ {:8.2} │", s.roll, w.roll, s.roll * w.roll);
    println!("│ インロール (3g外→内)        │ {:6.2} │ {:6.1} │ {:8.2} │", s.inroll, w.inroll, s.inroll * w.inroll);
    println!("│ アルペジオ (3g隣接指)       │ {:6.2} │ {:6.1} │ {:8.2} │", s.arpeggio, w.arpeggio, s.arpeggio * w.arpeggio);
    println!("│ 覚えやすさ (レイヤー一貫)   │ {:6.2} │ {:6.1} │ {:8.2} │", s.memorability, w.memorability, s.memorability * w.memorability);
    println!("│ シフトバランス (☆★均等)    │ {:6.2} │ {:6.1} │ {:8.2} │", s.shift_balance, w.shift_balance, s.shift_balance * w.shift_balance);
    println!("├─────────────────────────────┼────────┼────────┼──────────┤");
    println!("│ Bonus Total                 │        │        │ {:8.2} │", additive_bonus);
    println!("└─────────────────────────────┴────────┴────────┴──────────┘");
    
    println!("\nFinal: {:.2} × (1 + {:.2}/{:.0}) = {:.2}", core_multiplier, additive_bonus, bonus_scale, fitness);
    
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
    
    // Shift key positions: d (row=1, col=2) = ☆シフトキー, k (row=1, col=7) = ★シフトキー
    const SHIFT_A_POS: (usize, usize) = (1, 2); // d key: ☆シフト
    const SHIFT_B_POS: (usize, usize) = (1, 7); // k key: ★シフト
    
    // Build keys array with ☆ and ★ markers
    let mut keys_json = Vec::new();
    let mut conversion: HashMap<String, serde_json::Value> = HashMap::new();
    
    for row in 0..3 {
        let mut row_keys = Vec::new();
        for col in 0..10 {
            let c0 = layout.layers[0][row][col];
            let c1 = layout.layers[1][row][col];
            let c2 = layout.layers[2][row][col];
            
            // Override shift key positions with markers
            let (legend0, legend1, legend2) = if (row, col) == SHIFT_A_POS {
                ("☆".to_string(), c1.to_string(), c2.to_string())
            } else if (row, col) == SHIFT_B_POS {
                ("★".to_string(), c1.to_string(), c2.to_string())
            } else {
                (c0.to_string(), c1.to_string(), c2.to_string())
            };
            
            let mut key = serde_json::json!({
                "id": key_ids[row][col],
                "legend": [legend0, legend1, legend2],
                "size": 1,
                "finger": finger_map[row][col],
            });
            
            if home_mask[row][col] {
                key["home"] = serde_json::json!(true);
            }
            
            row_keys.push(key);
            
            // Build conversion mapping
            let key_id = key_ids[row][col];
            
            // Layer 0: 無シフト (skip shift key positions)
            if (row, col) != SHIFT_A_POS && (row, col) != SHIFT_B_POS && c0 != '　' && c0 != '\0' {
                conversion.insert(c0.to_string(), serde_json::json!({
                    "keys": [key_id],
                    "shift": [],
                    "type": "douji"
                }));
            }
            
            // Layer 1: ☆シフト (d key)
            if c1 != '　' && c1 != '\0' {
                conversion.insert(c1.to_string(), serde_json::json!({
                    "keys": [key_id],
                    "shift": ["d"],
                    "type": "douji"
                }));
            }
            
            // Layer 2: ★シフト (k key)
            if c2 != '　' && c2 != '\0' {
                conversion.insert(c2.to_string(), serde_json::json!({
                    "keys": [key_id],
                    "shift": ["k"],
                    "type": "douji"
                }));
            }
        }
        keys_json.push(row_keys);
    }
    
    // Arpeggio patterns (same as used in Svelte layouts)
    let arpeggio = vec![
        // Left hand - adjacent fingers on same row
        vec![vec![0, 1], vec![0, 2]], vec![vec![0, 2], vec![0, 3]], vec![vec![0, 1], vec![0, 3]],
        vec![vec![1, 1], vec![1, 2]], vec![vec![1, 2], vec![1, 3]], vec![vec![1, 1], vec![1, 3]],
        vec![vec![2, 1], vec![2, 2]], vec![vec![2, 2], vec![2, 3]], vec![vec![2, 1], vec![2, 3]],
        // Left hand - diagonal patterns
        vec![vec![0, 1], vec![1, 2]], vec![vec![0, 2], vec![1, 3]], vec![vec![1, 1], vec![2, 2]], vec![vec![1, 2], vec![2, 3]],
        // Right hand - adjacent fingers on same row
        vec![vec![0, 7], vec![0, 8]], vec![vec![0, 6], vec![0, 7]], vec![vec![0, 6], vec![0, 8]],
        vec![vec![1, 7], vec![1, 8]], vec![vec![1, 6], vec![1, 7]], vec![vec![1, 6], vec![1, 8]],
        vec![vec![2, 7], vec![2, 8]], vec![vec![2, 6], vec![2, 7]], vec![vec![2, 6], vec![2, 8]],
        // Right hand - diagonal patterns
        vec![vec![0, 6], vec![1, 7]], vec![vec![0, 7], vec![1, 8]], vec![vec![1, 6], vec![2, 7]], vec![vec![1, 7], vec![2, 8]],
    ];
    
    let analyzer_json = serde_json::json!({
        "name": format!("GA Optimized Layout (seed: {}, gen: {})", args.seed, args.generations),
        "remark": format!("Generated by GA optimizer. Fitness: {:.4}", layout.fitness),
        "keys": keys_json,
        "arpeggio": arpeggio,
        "conversion": conversion
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
