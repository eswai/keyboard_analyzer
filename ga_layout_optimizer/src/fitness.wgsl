// WGSL Compute Shader for Keyboard Layout Fitness Evaluation
// Evaluates multiple layouts in parallel on GPU

struct GpuLayout {
    chars0: array<u32, 32>,  // First 32 chars
    chars1: array<u32, 32>,  // Next 32 chars  
    chars2: array<u32, 32>,  // Last 32 chars (only 26 used, rest padding)
}

struct GpuCharFreq {
    char_idx: u32,
    frequency: u32,
}

struct GpuBigramFreq {
    char1_idx: u32,
    char2_idx: u32,
    frequency: u32,
    _padding: u32,
}

struct GpuKeyProps {
    weight: f32,
    finger: u32,
    is_home: u32,
    is_left: u32,
}

struct GpuFitnessResult {
    total_keystrokes: f32,
    home_keystrokes: f32,
    shifted_keystrokes: f32,
    single_keystrokes: f32,
    row_skips: f32,
    same_finger: f32,
    alternating: f32,
    total_chars: f32,
    bigram_total: f32,
    _padding: array<f32, 7>,
}

@group(0) @binding(0) var<storage, read> layouts: array<GpuLayout>;
@group(0) @binding(1) var<storage, read> char_freqs: array<GpuCharFreq>;
@group(0) @binding(2) var<storage, read> bigram_freqs: array<GpuBigramFreq>;
@group(0) @binding(3) var<storage, read> key_props: array<GpuKeyProps>;
@group(0) @binding(4) var<storage, read_write> results: array<GpuFitnessResult>;

// Get character at index from layout (handles split arrays)
fn get_layout_char(layout_idx: u32, idx: u32) -> u32 {
    let kb_layout = layouts[layout_idx];
    if (idx < 32u) {
        return kb_layout.chars0[idx];
    } else if (idx < 64u) {
        return kb_layout.chars1[idx - 32u];
    } else {
        return kb_layout.chars2[idx - 64u];
    }
}

// Find character position in layout
// Returns flat index (0-89), or 255 if not found
fn find_char_pos(layout_idx: u32, char_idx: u32) -> u32 {
    for (var i: u32 = 0u; i < 90u; i = i + 1u) {
        if (get_layout_char(layout_idx, i) == char_idx) {
            return i;
        }
    }
    return 255u;
}

// Get layer from flat index
fn get_layer(idx: u32) -> u32 {
    return idx / 30u;
}

// Get row from flat index
fn get_row(idx: u32) -> u32 {
    return (idx % 30u) / 10u;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let layout_idx = global_id.x;
    
    // Check bounds
    if (layout_idx >= arrayLength(&results)) {
        return;
    }
    
    var result: GpuFitnessResult;
    result.total_keystrokes = 0.0;
    result.home_keystrokes = 0.0;
    result.shifted_keystrokes = 0.0;
    result.single_keystrokes = 0.0;
    result.row_skips = 0.0;
    result.same_finger = 0.0;
    result.alternating = 0.0;
    result.total_chars = 0.0;
    result.bigram_total = 0.0;
    
    // Process character frequencies
    let num_char_freqs = arrayLength(&char_freqs);
    for (var i: u32 = 0u; i < num_char_freqs; i = i + 1u) {
        let cf = char_freqs[i];
        let pos = find_char_pos(layout_idx, cf.char_idx);
        
        if (pos < 255u) {
            let props = key_props[pos];
            let count = f32(cf.frequency);
            
            result.total_chars = result.total_chars + count;
            result.total_keystrokes = result.total_keystrokes + props.weight * count;
            
            if (props.is_home == 1u) {
                result.home_keystrokes = result.home_keystrokes + count;
            }
            
            let layer = get_layer(pos);
            if (layer > 0u) {
                result.shifted_keystrokes = result.shifted_keystrokes + count;
            } else {
                result.single_keystrokes = result.single_keystrokes + count;
            }
        }
    }
    
    // Process bigram frequencies
    let num_bigram_freqs = arrayLength(&bigram_freqs);
    for (var i: u32 = 0u; i < num_bigram_freqs; i = i + 1u) {
        let bf = bigram_freqs[i];
        let pos1 = find_char_pos(layout_idx, bf.char1_idx);
        let pos2 = find_char_pos(layout_idx, bf.char2_idx);
        
        if (pos1 < 255u && pos2 < 255u) {
            let props1 = key_props[pos1];
            let props2 = key_props[pos2];
            let count = f32(bf.frequency);
            
            result.bigram_total = result.bigram_total + count;
            
            // Same finger penalty
            if (props1.finger == props2.finger && pos1 != pos2) {
                result.same_finger = result.same_finger + count;
                
                // Row skip (段飛ばし)
                let row1 = get_row(pos1);
                let row2 = get_row(pos2);
                var row_diff: i32;
                if (row1 > row2) {
                    row_diff = i32(row1 - row2);
                } else {
                    row_diff = i32(row2 - row1);
                }
                if (row_diff >= 2) {
                    result.row_skips = result.row_skips + count;
                }
            }
            
            // Alternating hands
            if (props1.is_left != props2.is_left) {
                result.alternating = result.alternating + count;
            }
        }
    }
    
    results[layout_idx] = result;
}
