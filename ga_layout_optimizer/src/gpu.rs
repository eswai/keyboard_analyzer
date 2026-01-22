//! GPU-accelerated fitness evaluation using WGPU

use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

// ============================================================================
// GPU Data Structures
// ============================================================================

/// Layout data packed for GPU (must be Pod + Zeroable)
/// Split into smaller arrays to satisfy bytemuck constraints
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuLayout {
    /// Flattened layout split into 3 parts of 32 elements each
    pub chars0: [u32; 32], // First 32 chars
    pub chars1: [u32; 32], // Next 32 chars
    pub chars2: [u32; 32], // Last 26 chars + padding
}

/// Character frequency data for GPU
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuCharFreq {
    pub char_idx: u32,
    pub frequency: u32,
}

/// Bigram frequency data for GPU
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuBigramFreq {
    pub char1_idx: u32,
    pub char2_idx: u32,
    pub frequency: u32,
    pub _padding: u32,
}

/// Key properties for GPU
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuKeyProps {
    pub weight: f32,
    pub finger: u32,
    pub is_home: u32,
    pub is_left: u32,
}

/// Fitness result from GPU
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct GpuFitnessResult {
    pub total_keystrokes: f32,
    pub home_keystrokes: f32,
    pub shifted_keystrokes: f32,
    pub single_keystrokes: f32,
    pub row_skips: f32,
    pub same_finger: f32,
    pub alternating: f32,
    pub total_chars: f32,
    pub bigram_total: f32,
    pub _padding: [f32; 7], // Pad to 64 bytes
}

// ============================================================================
// GPU Context
// ============================================================================

pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    compute_pipeline: wgpu::ComputePipeline,
    
    // Buffers
    layout_buffer: wgpu::Buffer,
    char_freq_buffer: wgpu::Buffer,
    bigram_freq_buffer: wgpu::Buffer,
    key_props_buffer: wgpu::Buffer,
    result_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    
    // Bind group
    bind_group: wgpu::BindGroup,
    
    // Sizes
    num_layouts: u32,
    num_char_freqs: u32,
    num_bigram_freqs: u32,
}

impl GpuContext {
    pub async fn new(
        max_layouts: usize,
        char_freqs: &[GpuCharFreq],
        bigram_freqs: &[GpuBigramFreq],
        key_props: &[GpuKeyProps; 96],
    ) -> Result<Self, String> {
        // Request GPU instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("Failed to find GPU adapter")?;
        
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: Some("GA Layout Optimizer"),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .map_err(|e| format!("Failed to create device: {}", e))?;
        
        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Fitness Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fitness.wgsl").into()),
        });
        
        // Create buffers
        let layout_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Layout Buffer"),
            size: (std::mem::size_of::<GpuLayout>() * max_layouts) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let char_freq_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Char Freq Buffer"),
            contents: bytemuck::cast_slice(char_freqs),
            usage: wgpu::BufferUsages::STORAGE,
        });
        
        let bigram_freq_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bigram Freq Buffer"),
            contents: bytemuck::cast_slice(bigram_freqs),
            usage: wgpu::BufferUsages::STORAGE,
        });
        
        let key_props_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Key Props Buffer"),
            contents: bytemuck::cast_slice(key_props),
            usage: wgpu::BufferUsages::STORAGE,
        });
        
        let result_size = std::mem::size_of::<GpuFitnessResult>() * max_layouts;
        let result_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Result Buffer"),
            size: result_size as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: result_size as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Fitness Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        
        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fitness Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: layout_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: char_freq_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: bigram_freq_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: key_props_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: result_buffer.as_entire_binding(),
                },
            ],
        });
        
        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Fitness Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        // Create compute pipeline
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Fitness Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        
        Ok(Self {
            device,
            queue,
            compute_pipeline,
            layout_buffer,
            char_freq_buffer,
            bigram_freq_buffer,
            key_props_buffer,
            result_buffer,
            staging_buffer,
            bind_group,
            num_layouts: max_layouts as u32,
            num_char_freqs: char_freqs.len() as u32,
            num_bigram_freqs: bigram_freqs.len() as u32,
        })
    }
    
    /// Evaluate multiple layouts on GPU
    pub fn evaluate_batch(&self, layouts: &[GpuLayout]) -> Vec<GpuFitnessResult> {
        let num_layouts = layouts.len();
        
        // Upload layouts to GPU
        self.queue.write_buffer(&self.layout_buffer, 0, bytemuck::cast_slice(layouts));
        
        // Create command encoder
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Fitness Compute Encoder"),
        });
        
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Fitness Compute Pass"),
                timestamp_writes: None,
            });
            
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.bind_group, &[]);
            compute_pass.dispatch_workgroups((num_layouts as u32 + 63) / 64, 1, 1);
        }
        
        // Copy results to staging buffer
        let result_size = std::mem::size_of::<GpuFitnessResult>() * num_layouts;
        encoder.copy_buffer_to_buffer(
            &self.result_buffer,
            0,
            &self.staging_buffer,
            0,
            result_size as u64,
        );
        
        // Submit and wait
        self.queue.submit(std::iter::once(encoder.finish()));
        
        // Read back results
        let buffer_slice = self.staging_buffer.slice(..result_size as u64);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });
        
        self.device.poll(wgpu::Maintain::Wait);
        receiver.recv().unwrap().unwrap();
        
        let data = buffer_slice.get_mapped_range();
        let results: Vec<GpuFitnessResult> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        self.staging_buffer.unmap();
        
        results
    }
}

/// Convert character frequencies to GPU format
pub fn prepare_char_freqs(char_freq: &HashMap<char, usize>, char_to_idx: &HashMap<char, u32>) -> Vec<GpuCharFreq> {
    char_freq
        .iter()
        .filter_map(|(c, &freq)| {
            char_to_idx.get(c).map(|&idx| GpuCharFreq {
                char_idx: idx,
                frequency: freq as u32,
            })
        })
        .collect()
}

/// Convert bigram frequencies to GPU format
pub fn prepare_bigram_freqs(
    bigram_freq: &HashMap<(char, char), usize>,
    char_to_idx: &HashMap<char, u32>,
) -> Vec<GpuBigramFreq> {
    bigram_freq
        .iter()
        .filter_map(|((c1, c2), &freq)| {
            match (char_to_idx.get(c1), char_to_idx.get(c2)) {
                (Some(&idx1), Some(&idx2)) => Some(GpuBigramFreq {
                    char1_idx: idx1,
                    char2_idx: idx2,
                    frequency: freq as u32,
                    _padding: 0,
                }),
                _ => None,
            }
        })
        .collect()
}

/// Prepare key properties for GPU
pub fn prepare_key_props() -> [GpuKeyProps; 96] {
    const MY_WEIGHTS: [[f32; 10]; 3] = [
        [3.5, 2.0, 2.0, 2.0, 3.0, 3.0, 2.0, 2.0, 2.0, 3.5],
        [1.5, 1.0, 1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.5],
        [3.5, 2.0, 2.0, 2.0, 3.0, 3.0, 2.0, 2.0, 2.0, 3.5],
    ];
    
    const FINGER_MAP: [[u32; 10]; 3] = [
        [0, 1, 2, 3, 3, 6, 6, 7, 8, 9],
        [0, 1, 2, 3, 3, 6, 6, 7, 8, 9],
        [0, 1, 2, 3, 3, 6, 6, 7, 8, 9],
    ];
    
    const HOME_POSITIONS: [[bool; 10]; 3] = [
        [false, false, false, false, false, false, false, false, false, false],
        [true, true, true, true, false, false, true, true, true, true],
        [false, false, false, false, false, false, false, false, false, false],
    ];
    
    const SHIFT_WEIGHTS: [f32; 3] = [0.0, 3.0, 3.0];
    
    let mut props = [GpuKeyProps {
        weight: 0.0,
        finger: 0,
        is_home: 0,
        is_left: 0,
    }; 96]; // 3 x 32 for alignment with GpuLayout
    
    for layer in 0..3 {
        for row in 0..3 {
            for col in 0..10 {
                let idx = layer * 30 + row * 10 + col;
                let finger = FINGER_MAP[row][col];
                props[idx] = GpuKeyProps {
                    weight: MY_WEIGHTS[row][col] + SHIFT_WEIGHTS[layer],
                    finger,
                    is_home: if HOME_POSITIONS[row][col] { 1 } else { 0 },
                    is_left: if finger < 5 { 1 } else { 0 },
                };
            }
        }
    }
    
    props
}
