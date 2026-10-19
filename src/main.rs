use anyhow::Result;
use clidraw::app::App;
use clidraw::store::Store;
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{cursor, execute};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdout, Stdout, Write};
use std::panic::{self, AssertUnwindSafe};
use std::time::{Duration, Instant};

mod probe;

fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(
        stdout(),
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen,
        cursor::Show
    );
}

fn run(app: &mut App, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    while !app.quit {
        // Terminals without synchronized output ignore the unknown private mode.
        write!(terminal.backend_mut(), "\x1b[?2026h")?;
        terminal.draw(|f| {
            let area = f.area();
            app.render(f.buffer_mut(), area)
        })?;
        write!(terminal.backend_mut(), "\x1b[?2026l")?;
        terminal.backend_mut().flush()?;
        if event::poll(Duration::from_millis(250))? {
            // Mode 1003 floods the loop with Moved events; drain, then redraw once.
            loop {
                app.on_event(event::read()?, Instant::now());
                if app.quit || !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        app.tick(Instant::now());
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("probe") {
        return probe::run();
    }
    let (w, h) = crossterm::terminal::size()?;
    let mut app = App::open(Store::from_env()?, args.into_iter().next(), (w, h))?;

    enable_raw_mode()?;
    execute!(
        stdout(),
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore();
        default_hook(info);
    }));
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let result = panic::catch_unwind(AssertUnwindSafe(|| run(&mut app, &mut terminal)));
    restore();
    app.save();
    match result {
        Ok(r) => r,
        Err(payload) => panic::resume_unwind(payload),
    }
}
