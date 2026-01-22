//! TUI module for real-time visualization of GA optimization

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::Line,
    widgets::{
        Axis, Block, Borders, Chart, Dataset, Gauge, GraphType, List, ListItem, Paragraph, Wrap,
    },
    Frame, Terminal,
};
use std::{
    io,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::{EvaluationScores, Layout as KeyboardLayout};

/// Shared state for TUI updates
#[derive(Clone)]
pub struct TuiState {
    pub generation: usize,
    pub max_generations: usize,
    pub best_fitness: f64,
    pub fitness_history: Vec<(f64, f64)>, // (generation, fitness)
    pub current_layout: Option<KeyboardLayout>,
    pub scores: Option<EvaluationScores>,
}

impl TuiState {
    pub fn new(max_generations: usize) -> Self {
        Self {
            generation: 0,
            max_generations,
            best_fitness: 0.0,
            fitness_history: Vec::new(),
            current_layout: None,
            scores: None,
        }
    }

    pub fn update(
        &mut self,
        generation: usize,
        fitness: f64,
        layout: &KeyboardLayout,
        scores: &EvaluationScores,
    ) {
        self.generation = generation;
        self.best_fitness = fitness;
        self.fitness_history.push((generation as f64, fitness));
        
        // Keep all history (no removal) to show full progress
        // For large datasets, ratatui will automatically sample
        
        self.current_layout = Some(layout.clone());
        self.scores = Some(scores.clone());
    }
}

pub fn run_tui(state: Arc<Mutex<TuiState>>) -> Result<(), io::Error> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, state);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    state: Arc<Mutex<TuiState>>,
) -> io::Result<()> {
    loop {
        let state_clone = state.lock().unwrap().clone();
        terminal.draw(|f| ui(f, &state_clone))?;

        // Always check for quit key ('q' or Esc)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    return Ok(());
                }
            }
        }
    }
}

fn ui(f: &mut Frame, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Progress bar
            Constraint::Length(12), // Keyboard layout
            Constraint::Min(10),    // Graph
            Constraint::Length(10), // Metrics
        ])
        .split(f.area());

    // Progress bar
    render_progress(f, chunks[0], state);

    // Keyboard layout
    render_keyboard(f, chunks[1], state);

    // Fitness graph
    render_graph(f, chunks[2], state);

    // Metrics
    render_metrics(f, chunks[3], state);
}

fn render_progress(f: &mut Frame, area: Rect, state: &TuiState) {
    let progress = (state.generation as f64 / state.max_generations as f64).min(1.0);
    let is_complete = state.generation >= state.max_generations;
    
    let label = if is_complete {
        format!(
            "COMPLETE! Gen {}/{} | Best Fitness: {:.4} [Press 'q' to exit]",
            state.generation, state.max_generations, state.best_fitness
        )
    } else {
        format!(
            "Gen {}/{} | Best Fitness: {:.4}",
            state.generation, state.max_generations, state.best_fitness
        )
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Progress"))
        .gauge_style(
            Style::default()
                .fg(if is_complete { Color::Yellow } else { Color::Green })
                .add_modifier(Modifier::BOLD),
        )
        .ratio(progress)
        .label(label);

    f.render_widget(gauge, area);
}

fn render_keyboard(f: &mut Frame, area: Rect, state: &TuiState) {
    let layout_text = if let Some(ref layout) = state.current_layout {
        format_keyboard_layout(layout)
    } else {
        "Initializing...".to_string()
    };

    let paragraph = Paragraph::new(layout_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Current Best Layout (Global Best across all runs)"),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn format_keyboard_layout(layout: &KeyboardLayout) -> String {
    let mut result = String::new();
    
    for layer in 0..3 {
        let layer_name = match layer {
            0 => "無シフト ",
            1 => "☆シフト ",
            2 => "★シフト ",
            _ => "",
        };
        result.push_str(&format!("{}  ", layer_name));
        
        for row in 0..3 {
            for col in 0..10 {
                let c = layout.layers[layer][row][col];
                if c == '　' || c == '\0' {
                    result.push(' ');
                } else {
                    result.push(c);
                }
                result.push(' ');
            }
            if row < 2 {
                result.push_str("\n           ");
            }
        }
        result.push('\n');
    }
    
    result
}

fn render_graph(f: &mut Frame, area: Rect, state: &TuiState) {
    if state.fitness_history.is_empty() {
        let paragraph = Paragraph::new("Waiting for data...")
            .block(Block::default().borders(Borders::ALL).title("Fitness History"));
        f.render_widget(paragraph, area);
        return;
    }

    // Fixed X-axis: always show full generation range 0 to max_generations
    let min_gen = 0.0;
    let max_gen = state.max_generations as f64;
    
    // Fixed Y-axis range: 0-100 for full view
    let min_fitness = 0.0;
    let max_fitness = 100.0;

    let datasets = vec![Dataset::default()
        .name("Fitness")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(Color::Cyan))
        .data(&state.fitness_history)];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Fitness History (Gen 0 to Max)"),
        )
        .x_axis(
            Axis::default()
                .title("Generation")
                .style(Style::default().fg(Color::Gray))
                .bounds([min_gen, max_gen])
                .labels(vec![
                    Line::from("0"),
                    Line::from(format!("{}", state.max_generations / 2)),
                    Line::from(format!("{}", state.max_generations)),
                ]),
        )
        .y_axis(
            Axis::default()
                .title("Fitness")
                .style(Style::default().fg(Color::Gray))
                .bounds([min_fitness, max_fitness])
                .labels(vec![
                    Line::from("0"),
                    Line::from("50"),
                    Line::from("100"),
                ]),
        );

    f.render_widget(chart, area);
}

fn render_metrics(f: &mut Frame, area: Rect, state: &TuiState) {
    let items: Vec<ListItem> = if let Some(ref scores) = state.scores {
        vec![
            ListItem::new(format!("同指連続低: {:.2}%", scores.same_finger)),
            ListItem::new(format!("段飛ばし少: {:.2}%", scores.row_skip)),
            ListItem::new(format!("ホームポジ率: {:.2}%", scores.home_position)),
            ListItem::new(format!("総打鍵コスト: {:.2}%", scores.total_keystrokes)),
            ListItem::new(format!("左右交互: {:.2}%", scores.alternating)),
            ListItem::new(format!("単打鍵率: {:.2}%", scores.single_key)),
            ListItem::new(format!("リダイレクト: {:.2}", scores.redirect_low)),
            ListItem::new(format!("Colemak類似: {:.2}", scores.colemak_similarity)),
        ]
    } else {
        vec![ListItem::new("Waiting for scores...")]
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Core Metrics"),
    );

    f.render_widget(list, area);
}
