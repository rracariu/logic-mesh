//! TUI demo of a running Logic Mesh `Program`.
//!
//! Loads a small `Program` (defined inline below; same shape as
//! the JSON you save out of the web editor), runs it on the
//! `MultiThreadedEngine`, subscribes to the engine's watch channel,
//! and renders a live table of every block's current state and pin
//! values with ratatui.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example tui_runner --features multi-threaded
//! ```
//!
//! Press `q` to quit.
//!
//! The point of this example is to show that the same engine drives
//! both the browser editor and a terminal UI. The watcher
//! subscription (`EngineClient::watch`) is the single primitive both
//! UIs build on — every change-of-value (pin value, fault state)
//! arrives as a `WatchMessage` over an unbounded mpsc.

use std::collections::BTreeMap;
use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block as TuiBlock, Borders, Cell, Paragraph, Row, Table},
};
use uuid::Uuid;

use logic_mesh::Value;
use logic_mesh::base::engine::messages::{ChangeSource, WatchMessage};
use logic_mesh::base::program::{Program, ProgramBuilder};
use logic_mesh::multi_threaded::MultiThreadedEngine;
use logic_mesh::prelude::*;
use logic_mesh::sync::mpsc;

type Backend = CrosstermBackend<Stdout>;

/// Per-block state the TUI tracks. Updated from each `WatchMessage`.
#[derive(Default, Clone)]
struct BlockRow {
    name: String,
    state: String,
    fault_reason: Option<String>,
    inputs: BTreeMap<String, Value>,
    outputs: BTreeMap<String, Value>,
    last_update: Option<Instant>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    // ----- 1. Build a small Program in code (same shape as the JSON
    //         shape the web editor produces) -----
    let program = sample_program()?;

    // ----- 2. Spin up the engine -----
    let mut eng = MultiThreadedEngine::new();
    // A typed handle to the engine, usable from any task once it runs.
    let client = eng.client();

    // The engine task owns the engine; `run()` only returns on a
    // shutdown request.
    let engine_handle = tokio::spawn(async move {
        eng.run().await;
    });

    // ----- 3. Subscribe to watch events BEFORE loading the program
    //         so we don't miss the first round of COV notifications -----
    let mut watch = client.watch().await?;

    // ----- 4. Load the program -----
    client.load_program(program.clone()).await?;

    // Seed the table with every block name from the program.
    let mut rows: BTreeMap<Uuid, BlockRow> = BTreeMap::new();
    for (id_str, pb) in &program.blocks {
        if let Ok(id) = Uuid::parse_str(id_str) {
            rows.insert(
                id,
                BlockRow {
                    name: pb.label.clone().unwrap_or_else(|| pb.name.clone()),
                    state: "running".to_string(),
                    ..Default::default()
                },
            );
        }
    }

    // ----- 5. Drive the terminal -----
    let mut terminal = init_terminal()?;
    let result = run_ui(&mut terminal, &mut rows, &mut watch.notifications).await;
    restore_terminal(&mut terminal)?;

    // ----- 6. Shutdown -----
    client.shutdown().await?;
    let _ = engine_handle.await;

    result
}

/// Main UI loop. Returns when the user presses 'q' (or on a terminal
/// error).
async fn run_ui(
    terminal: &mut Terminal<Backend>,
    rows: &mut BTreeMap<Uuid, BlockRow>,
    watch_rx: &mut mpsc::UnboundedReceiver<WatchMessage>,
) -> anyhow::Result<()> {
    // Poll for terminal input at ~30Hz, redraw on every watch event
    // or input poll cycle.
    let mut tick = tokio::time::interval(Duration::from_millis(33));
    loop {
        tokio::select! {
            // Watch events: update rows.
            Some(msg) = watch_rx.recv() => {
                apply_watch_message(rows, msg);
                draw(terminal, rows)?;
            }
            // Periodic terminal poll: redraw + check keyboard.
            _ = tick.tick() => {
                if event::poll(Duration::from_millis(0))?
                    && let Event::Key(k) = event::read()?
                    && (k.code == KeyCode::Char('q') || k.code == KeyCode::Esc)
                {
                    return Ok(());
                }
                draw(terminal, rows)?;
            }
        }
    }
}

fn apply_watch_message(rows: &mut BTreeMap<Uuid, BlockRow>, msg: WatchMessage) {
    let entry = rows.entry(msg.block_id).or_default();
    entry.state = msg.state.label().to_string();
    entry.fault_reason = msg.state.fault_reason().map(|s| s.to_string());
    entry.last_update = Some(Instant::now());
    for change in msg.changes {
        match change {
            ChangeSource::Input(name, value) => {
                entry.inputs.insert(name, value);
            }
            ChangeSource::Output(name, value) => {
                entry.outputs.insert(name, value);
            }
            // `ChangeSource` is non-exhaustive: skip sources this
            // runner does not know.
            _ => {}
        }
    }
}

fn draw(terminal: &mut Terminal<Backend>, rows: &BTreeMap<Uuid, BlockRow>) -> anyhow::Result<()> {
    terminal.draw(|f| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(f.area());

        let header = Paragraph::new(Line::from(vec![
            Span::styled("Logic Mesh", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  —  press "),
            Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to quit"),
        ]))
        .block(TuiBlock::default().borders(Borders::ALL));
        f.render_widget(header, chunks[0]);

        let header_row = Row::new(vec!["block", "state", "inputs", "outputs"])
            .style(Style::default().add_modifier(Modifier::BOLD));

        let body: Vec<Row> = rows
            .values()
            .map(|r| {
                let state_style = if r.state == "fault" {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default()
                };
                Row::new(vec![
                    Cell::from(r.name.clone()),
                    Cell::from(if let Some(reason) = &r.fault_reason {
                        format!("{}: {}", r.state, reason)
                    } else {
                        r.state.clone()
                    })
                    .style(state_style),
                    Cell::from(fmt_pins(&r.inputs)),
                    Cell::from(fmt_pins(&r.outputs)),
                ])
            })
            .collect();

        let table = Table::new(
            body,
            [
                Constraint::Length(20),
                Constraint::Length(30),
                Constraint::Percentage(40),
                Constraint::Percentage(40),
            ],
        )
        .header(header_row)
        .block(TuiBlock::default().borders(Borders::ALL).title("Blocks"));
        f.render_widget(table, chunks[1]);
    })?;
    Ok(())
}

/// Compact one-line rendering of a pin map for the table cell.
fn fmt_pins(pins: &BTreeMap<String, Value>) -> String {
    if pins.is_empty() {
        return String::new();
    }
    pins.iter()
        .map(|(name, v)| format!("{name}={}", fmt_value(v)))
        .collect::<Vec<_>>()
        .join("  ")
}

fn fmt_value(v: &Value) -> String {
    if let Ok(n) = TryInto::<f64>::try_into(v) {
        format!("{n:.2}")
    } else {
        format!("{v:?}")
    }
}

fn init_terminal() -> io::Result<Terminal<Backend>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut Terminal<Backend>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Build a small `Program` inline: two `SineWave`s feeding an `Add`.
/// Mirrors what you'd save out of the web editor.
///
/// ## Why no `MovingAverage`/`Ema`/etc. as the leaf
///
/// Periodic and stateful blocks (SineWave, MovingAverage, EMA,
/// Derivative, PID, Sequencer, ...) early-return from `execute` when
/// their output has no downstream link:
///
/// ```ignore
/// if !self.out.is_connected() { return; }
/// ```
///
/// That's a deliberate perf optimization — they don't waste cycles
/// computing values nobody reads. A watcher subscription does *not*
/// count as a "connection" for that check (it only looks at explicit
/// `output -> input` links between blocks). So a periodic block used
/// as the final leaf of a graph is invisible to the TUI's watcher.
///
/// Workaround if you want one visible: add a downstream consumer
/// (e.g., another block) that reads the periodic block's output.
/// In this demo we keep things minimal and use `Add` (which has no
/// such guard) as the visible leaf.
fn sample_program() -> logic_mesh::Result<Program> {
    let mut builder =
        ProgramBuilder::new("sine + sum").description("Two sine sources feeding an adder.");

    let sine1 = builder
        .add_block("SineWave")?
        .label("fast sine")
        .position(0.0, 0.0)
        .input("freq", 50)?
        .input("amplitude", 3)?
        .finish();
    let sine2 = builder
        .add_block("SineWave")?
        .label("slow sine")
        .position(0.0, 100.0)
        .input("freq", 200)?
        .input("amplitude", 7)?
        .finish();
    let adder = builder
        .add_block("Add")?
        .label("sum")
        .position(200.0, 50.0)
        .finish();

    builder.link(sine1, "out", adder, "in0")?;
    builder.link(sine2, "out", adder, "in1")?;

    Ok(builder.build())
}
