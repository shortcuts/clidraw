use anyhow::Result;
use clidraw::app::App;
use clidraw::config::{Config, DEFAULT_TOML};
use clidraw::frame_writer::FrameWriter;
use clidraw::store::Store;
use clidraw::ui::theme;
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{cursor, execute};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdout, Stdout, Write};
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

mod bench;
mod probe;

static KEYBOARD_ENHANCED: AtomicBool = AtomicBool::new(false);
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn on_signal(_: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
    // crossterm's poll retries on EINTR but wakes on its SIGWINCH handler.
    unsafe { libc::kill(libc::getpid(), libc::SIGWINCH) };
}

#[cfg(unix)]
fn install_signal_handlers() {
    unsafe {
        let mut act: libc::sigaction = std::mem::zeroed();
        act.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as usize;
        libc::sigemptyset(&mut act.sa_mask);
        for sig in [libc::SIGHUP, libc::SIGTERM] {
            libc::sigaction(sig, &act, std::ptr::null_mut());
        }
    }
}

#[cfg(not(unix))]
fn install_signal_handlers() {}

fn restore() {
    let _ = disable_raw_mode();
    if KEYBOARD_ENHANCED.swap(false, Ordering::SeqCst) {
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }
    let _ = execute!(
        stdout(),
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen,
        cursor::Show
    );
    // Pops the shape pushed at startup; terminals without OSC 22 ignore it.
    let _ = stdout()
        .write_all(b"\x1b]22;<\x1b\\")
        .and_then(|_| stdout().flush());
}

fn draw_frame(
    app: &mut App,
    out: &FrameWriter<Stdout>,
    terminal: &mut Terminal<CrosstermBackend<FrameWriter<Stdout>>>,
) -> Result<usize> {
    // Terminals without synchronized output ignore the unknown private mode.
    write!(terminal.backend_mut(), "\x1b[?2026h")?;
    if let Some(shape) = app.take_pointer() {
        write!(terminal.backend_mut(), "\x1b]22;={shape}\x1b\\")?;
    }
    if let Some(seq) = app.take_copy() {
        write!(terminal.backend_mut(), "{seq}")?;
    }
    terminal.draw(|f| {
        let area = f.area();
        app.render(f.buffer_mut(), area)
    })?;
    write!(terminal.backend_mut(), "\x1b[?2026l")?;
    out.send()?;
    app.dirty = false;
    Ok(out.take_bytes())
}

fn run(
    app: &mut App,
    out: &FrameWriter<Stdout>,
    terminal: &mut Terminal<CrosstermBackend<FrameWriter<Stdout>>>,
) -> Result<()> {
    while !app.quit && !SHUTDOWN.load(Ordering::SeqCst) {
        if app.dirty {
            let t0 = app.perf.is_some().then(Instant::now);
            let bytes = draw_frame(app, out, terminal)?;
            if let (Some(t0), Some(p)) = (t0, app.perf.as_mut()) {
                p.record_frame(t0.elapsed(), bytes);
            }
        }
        let ready = match app.next_wake(Instant::now()) {
            Some(timeout) => event::poll(timeout)?,
            None => true,
        };
        if ready {
            // Mode 1003 floods the loop with Moved events; drain, then redraw once.
            let mut queue = vec![event::read()?];
            while event::poll(Duration::ZERO)? {
                queue.push(event::read()?);
            }
            if let Some(p) = app.perf.as_mut() {
                p.events += queue.len();
            }
            app.on_events(queue, Instant::now());
        }
        app.tick(Instant::now());
    }
    Ok(())
}

const USAGE: &str = "Usage: clidraw [<name>]
       clidraw list|new|show|edit|import|export|template|templates ...
       clidraw mcp|config|probe|bench
       clidraw --help|-h|--version|-V";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--help" | "-h") => {
            println!("{USAGE}");
            return Ok(());
        }
        Some("--version" | "-V") => {
            println!("clidraw {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        _ => {}
    }
    if args.first().map(String::as_str) == Some("probe") {
        return probe::run();
    }
    if args.first().map(String::as_str) == Some("bench") {
        return bench::run();
    }
    if args.first().map(String::as_str) == Some("mcp") {
        let store = Store::from_env()?;
        return clidraw::mcp::serve(&store, std::io::stdin().lock(), &mut stdout().lock());
    }
    if args
        .first()
        .is_some_and(|a| clidraw::cli::COMMANDS.contains(&a.as_str()))
    {
        let store = Store::from_env()?;
        return clidraw::cli::run(&args, &store, &mut std::io::stdin(), &mut stdout().lock());
    }
    if args.first().map(String::as_str) == Some("config") {
        print!("{DEFAULT_TOML}");
        return Ok(());
    }
    let (w, h) = crossterm::terminal::size()?;
    let store = Store::from_env()?;
    let (config, warnings) = Config::load(store.dir());
    let setting = config.theme;
    let mut app = App::open_with(store, args.into_iter().next(), (w, h), config, warnings)?;

    enable_raw_mode()?;
    install_signal_handlers();
    execute!(
        stdout(),
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    stdout().write_all(b"\x1b]22;>default\x1b\\")?;
    app.session.theme = theme::resolve(setting, theme::Depth::from_env(), theme::detect);
    // Querying support needs raw mode; unsupported terminals keep the legacy encoding.
    if crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false) {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
        KEYBOARD_ENHANCED.store(true, Ordering::SeqCst);
    }
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore();
        default_hook(info);
    }));
    let out = FrameWriter::new(stdout());
    let mut terminal = Terminal::new(CrosstermBackend::new(out.handle()))?;
    let result = panic::catch_unwind(AssertUnwindSafe(|| run(&mut app, &out, &mut terminal)));
    restore();
    app.finish();
    match result {
        Ok(r) => r,
        Err(payload) => panic::resume_unwind(payload),
    }
}
