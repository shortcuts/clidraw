use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use std::io::stdout;

/// Prints every terminal event until `q`, to see what a terminal reports.
pub fn run() -> anyhow::Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    // The query needs raw mode, so it runs after enable_raw_mode.
    let enhanced = crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false);
    if enhanced {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
    }
    print!(
        "kitty keyboard protocol: {}\r\n",
        if enhanced { "yes" } else { "no" }
    );
    print!("clidraw probe: move, click, drag, scroll; press q to quit\r\n");
    let result = (|| -> anyhow::Result<()> {
        loop {
            let ev = event::read()?;
            print!("{ev:?}\r\n");
            if let Event::Key(k) = ev {
                if k.kind != KeyEventKind::Release && k.code == KeyCode::Char('q') {
                    return Ok(());
                }
            }
        }
    })();
    if enhanced {
        execute!(stdout(), PopKeyboardEnhancementFlags)?;
    }
    execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    result
}
