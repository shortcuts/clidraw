use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const ROWS: u16 = 30;
const COLS: u16 = 100;
const DA1_REPLY: &[u8] = b"\x1b[?62;c";

pub struct Term {
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    rx: Receiver<Vec<u8>>,
    parser: vt100::Parser,
    total: usize,
}

impl Term {
    pub fn spawn(tmp: &Path, args: &[&str]) -> Term {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: ROWS,
                cols: COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_clidraw"));
        cmd.args(args);
        cmd.env("CLIDRAW_HOME", tmp.join("c"));
        cmd.env("XDG_STATE_HOME", tmp.join("state"));
        cmd.env("TERM", "xterm-256color");
        cmd.env("NO_COLOR", "1");
        let child = pair.slave.spawn_command(cmd).unwrap();
        let mut reader = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n @ 1..) = reader.read(&mut buf) {
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        let mut t = Term {
            child,
            _master: pair.master,
            writer,
            rx,
            parser: vt100::Parser::new(
                NonZeroU16::new(ROWS).unwrap(),
                NonZeroU16::new(COLS).unwrap(),
                0,
            ),
            total: 0,
        };
        t.quiet();
        t
    }

    fn pump(&mut self) {
        while let Ok(chunk) = self.rx.try_recv() {
            self.total += chunk.len();
            // The app blocks 2 s at startup unless the terminal answers its DA1 query.
            if chunk.windows(3).any(|w| w == b"\x1b[c") {
                self.send(DA1_REPLY);
            }
            self.parser.process(&chunk);
        }
    }

    pub fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
    }

    pub fn key(&mut self, s: &str) {
        self.send(s.as_bytes());
    }

    fn mouse_bytes(btn: u8, col: u16, row: u16, press: bool) -> Vec<u8> {
        let end = if press { 'M' } else { 'm' };
        format!("\x1b[<{btn};{};{}{end}", col + 1, row + 1).into_bytes()
    }

    pub fn click(&mut self, col: u16, row: u16) {
        self.click_btn(0, col, row);
    }

    pub fn right_click(&mut self, col: u16, row: u16) {
        self.click_btn(2, col, row);
    }

    fn click_btn(&mut self, btn: u8, col: u16, row: u16) {
        let mut b = Self::mouse_bytes(btn, col, row, true);
        b.extend(Self::mouse_bytes(btn, col, row, false));
        self.send(&b);
        self.quiet();
    }

    pub fn double_click(&mut self, col: u16, row: u16) {
        let mut b = Vec::new();
        for _ in 0..2 {
            b.extend(Self::mouse_bytes(0, col, row, true));
            b.extend(Self::mouse_bytes(0, col, row, false));
        }
        self.send(&b);
        self.quiet();
    }

    pub fn drag(&mut self, from: (u16, u16), to: (u16, u16)) {
        let mut b = Self::mouse_bytes(0, from.0, from.1, true);
        b.extend(Self::mouse_bytes(32, to.0, to.1, true));
        b.extend(Self::mouse_bytes(0, to.0, to.1, false));
        self.send(&b);
        self.quiet();
    }

    pub fn press(&mut self, col: u16, row: u16) {
        self.send(&Self::mouse_bytes(0, col, row, true));
        self.quiet();
    }

    pub fn move_to(&mut self, col: u16, row: u16) {
        self.send(&Self::mouse_bytes(32, col, row, true));
        self.quiet();
    }

    pub fn release(&mut self, col: u16, row: u16) {
        self.send(&Self::mouse_bytes(0, col, row, false));
        self.quiet();
    }

    pub fn wheel_down(&mut self, col: u16, row: u16) {
        self.send(&Self::mouse_bytes(65, col, row, true));
        self.quiet();
    }

    pub fn wait_for(&mut self, text: &str) {
        let end = Instant::now() + Duration::from_secs(3);
        while !self.screen().contains(text) {
            assert!(
                Instant::now() < end,
                "timed out waiting for {text:?}\n{}",
                self.screen()
            );
            std::thread::sleep(Duration::from_millis(10));
            self.pump();
        }
    }

    pub fn find(&mut self, text: &str) -> (u16, u16) {
        self.pump();
        for (row, line) in self.screen().lines().enumerate() {
            if let Some(i) = line.find(text) {
                return (line[..i].chars().count() as u16, row as u16);
            }
        }
        panic!("{text:?} not on screen\n{}", self.screen());
    }

    pub fn screen(&self) -> String {
        self.parser.screen().contents()
    }

    pub fn canvas(&self) -> String {
        let s = self.screen();
        let body = &s[..s.rfind('\n').unwrap_or(0)];
        body.lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn quiet(&mut self) {
        let mut last = Instant::now();
        while last.elapsed() < Duration::from_millis(250) {
            std::thread::sleep(Duration::from_millis(10));
            let before = self.total;
            self.pump();
            if self.total != before {
                last = Instant::now();
            }
        }
    }

    pub fn bytes_in(&mut self, window: Duration) -> usize {
        self.pump();
        let before = self.total;
        std::thread::sleep(window);
        self.pump();
        self.total - before
    }

    pub fn draw_rect(&mut self) {
        self.key("r");
        self.drag((10, 8), (22, 13));
    }

    pub fn draw_rect_at(&mut self, from: (u16, u16), to: (u16, u16)) {
        self.key("r");
        self.drag(from, to);
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
