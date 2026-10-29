use crate::model::{Paint, FILL_ALPHA};
use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeSetting {
    Auto,
    Dark,
    Light,
}

impl Theme {
    pub fn accent(self) -> Color {
        match self {
            Theme::Dark => Color::Rgb(95, 215, 255),
            Theme::Light => Color::Rgb(0, 95, 135),
        }
    }

    fn background(self) -> (u8, u8, u8) {
        match self {
            Theme::Dark => (30, 30, 30),
            Theme::Light => (255, 255, 255),
        }
    }
}

/// How many colors the terminal shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    None,
    Ansi16,
    Ansi256,
    Truecolor,
}

impl Depth {
    pub fn detect(no_color: bool, colorterm: Option<&str>, term: Option<&str>) -> Depth {
        match (no_color, colorterm, term) {
            (true, ..) => Depth::None,
            (_, Some("truecolor" | "24bit"), _) => Depth::Truecolor,
            (_, _, Some(t)) if t.contains("256color") => Depth::Ansi256,
            (_, _, Some(t)) if !t.is_empty() && t != "dumb" => Depth::Ansi16,
            _ => Depth::None,
        }
    }

    pub fn from_env() -> Depth {
        Depth::detect(
            std::env::var_os("NO_COLOR").is_some(),
            std::env::var("COLORTERM").ok().as_deref(),
            std::env::var("TERM").ok().as_deref(),
        )
    }
}

pub fn colors(d: Depth) -> bool {
    d != Depth::None
}

pub fn accent(theme: Theme, d: Depth) -> Option<Color> {
    colors(d).then(|| theme.accent())
}

const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

const ANSI16: [((u8, u8, u8), Color); 16] = [
    ((0, 0, 0), Color::Black),
    ((205, 0, 0), Color::Red),
    ((0, 205, 0), Color::Green),
    ((205, 205, 0), Color::Yellow),
    ((0, 0, 238), Color::Blue),
    ((205, 0, 205), Color::Magenta),
    ((0, 205, 205), Color::Cyan),
    ((229, 229, 229), Color::Gray),
    ((127, 127, 127), Color::DarkGray),
    ((255, 0, 0), Color::LightRed),
    ((0, 255, 0), Color::LightGreen),
    ((255, 255, 0), Color::LightYellow),
    ((92, 92, 255), Color::LightBlue),
    ((255, 0, 255), Color::LightMagenta),
    ((0, 255, 255), Color::LightCyan),
    ((255, 255, 255), Color::White),
];

fn dist(a: (u8, u8, u8), b: (u8, u8, u8)) -> i32 {
    let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).pow(2);
    d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
}

/// Maps a truecolor to the nearest color the terminal depth can show.
pub fn downgrade(d: Depth, c: Color) -> Color {
    let Color::Rgb(r, g, b) = c else {
        return c;
    };
    let rgb = (r, g, b);
    match d {
        Depth::None | Depth::Truecolor => c,
        Depth::Ansi256 => {
            let level = |v: u8| {
                (0..6)
                    .min_by_key(|&i| i32::from(v).abs_diff(i32::from(CUBE[i])))
                    .unwrap()
            };
            let (ri, gi, bi) = (level(r), level(g), level(b));
            let cube = (CUBE[ri], CUBE[gi], CUBE[bi]);
            let mean = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
            let step = (0..24u8)
                .min_by_key(|&i| (i32::from(mean) - i32::from(8 + 10 * i)).abs())
                .unwrap();
            let gray = 8 + 10 * step;
            if dist(rgb, (gray, gray, gray)) < dist(rgb, cube) {
                Color::Indexed(232 + step)
            } else {
                Color::Indexed((16 + 36 * ri + 6 * gi + bi) as u8)
            }
        }
        Depth::Ansi16 => ANSI16.iter().min_by_key(|(v, _)| dist(rgb, *v)).unwrap().1,
    }
}

/// Light ink is the dark ink scaled down: every paint then keeps 4.5:1 on white.
pub fn ink(theme: Theme, p: Paint) -> Color {
    let (r, g, b) = p.rgb();
    match theme {
        Theme::Dark => Color::Rgb(r, g, b),
        Theme::Light => Color::Rgb(darken(r), darken(g), darken(b)),
    }
}

/// The default ink of a frame that has no color.
pub fn frame_ink(theme: Theme) -> Color {
    match theme {
        Theme::Dark => Color::Rgb(150, 150, 150),
        Theme::Light => Color::Rgb(118, 118, 118),
    }
}

fn darken(c: u8) -> u8 {
    (u16::from(c) * 45 / 100) as u8
}

/// The ink blended over the background: a terminal cell has no alpha.
pub fn fill(theme: Theme, p: Paint) -> Color {
    let (r, g, b) = p.rgb();
    let bg = theme.background();
    let mix = |c: u8, bg: u8| {
        ((u16::from(c) * u16::from(FILL_ALPHA) + u16::from(bg) * (100 - u16::from(FILL_ALPHA)))
            / 100) as u8
    };
    Color::Rgb(mix(r, bg.0), mix(g, bg.1), mix(b, bg.2))
}

/// Never calls `detected` unless the setting is `Auto` and colors are on.
pub fn resolve(setting: ThemeSetting, d: Depth, detected: impl FnOnce() -> Option<Theme>) -> Theme {
    match setting {
        ThemeSetting::Dark => Theme::Dark,
        ThemeSetting::Light => Theme::Light,
        ThemeSetting::Auto if colors(d) => detected().unwrap_or(Theme::Dark),
        ThemeSetting::Auto => Theme::Dark,
    }
}

/// Reads the `rgb:R/G/B` of an OSC 11 reply; each part has 1 to 4 hex digits.
pub fn parse_osc11(reply: &[u8]) -> Option<Theme> {
    const MARK: &[u8] = b"\x1b]11;rgb:";
    let at = reply.windows(MARK.len()).position(|w| w == MARK)? + MARK.len();
    let mut parts = reply[at..].split(|&b| b == b'/');
    let mut channel = || -> Option<f64> {
        let hex: Vec<u8> = parts
            .next()?
            .iter()
            .copied()
            .take_while(u8::is_ascii_hexdigit)
            .collect();
        let n = u32::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?;
        (1..=4)
            .contains(&hex.len())
            .then(|| f64::from(n) / f64::from(16u32.pow(hex.len() as u32) - 1))
    };
    let (r, g, b) = (channel()?, channel()?, channel()?);
    let brightness = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    Some(if brightness > 0.5 {
        Theme::Light
    } else {
        Theme::Dark
    })
}

/// Asks the terminal for its background. DA1 follows so terminals without OSC 11 answer at once.
#[cfg(unix)]
pub fn detect() -> Option<Theme> {
    use std::io::Write;
    let mut out = std::io::stdout();
    out.write_all(b"\x1b]11;?\x07\x1b[c").ok()?;
    out.flush().ok()?;
    parse_osc11(&read_reply(0, REPLY_TIMEOUT))
}

/// Over SSH the reply arrives late, and crossterm would read the leftover bytes as keys.
#[cfg(unix)]
const REPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

#[cfg(unix)]
fn read_reply(fd: i32, timeout: std::time::Duration) -> Vec<u8> {
    let deadline = std::time::Instant::now() + timeout;
    let mut reply = Vec::new();
    while !answered_da1(&reply) {
        let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) else {
            break;
        };
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `pfd` is one valid pollfd and the count says 1.
        if unsafe { libc::poll(&mut pfd, 1, left.as_millis() as i32 + 1) } <= 0 {
            break;
        }
        let mut chunk = [0u8; 128];
        // SAFETY: `chunk` is writable for its whole length.
        let n = unsafe { libc::read(fd, chunk.as_mut_ptr().cast(), chunk.len()) };
        if n <= 0 {
            break;
        }
        reply.extend_from_slice(&chunk[..n as usize]);
    }
    reply
}

#[cfg(unix)]
fn answered_da1(reply: &[u8]) -> bool {
    reply
        .windows(3)
        .position(|w| w == b"\x1b[?")
        .is_some_and(|i| reply[i..].contains(&b'c'))
}

#[cfg(not(unix))]
pub fn detect() -> Option<Theme> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: Color = Color::Rgb(30, 30, 30);
    const WHITE: Color = Color::Rgb(255, 255, 255);
    const TEXT: Color = Color::Rgb(229, 229, 229);
    const DARK_TEXT: Color = Color::Rgb(30, 30, 30);

    fn luminance(c: Color) -> f64 {
        let Color::Rgb(r, g, b) = c else {
            panic!("theme colors are rgb");
        };
        let lin = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }

    fn contrast(a: Color, b: Color) -> f64 {
        let (hi, lo) = (
            luminance(a).max(luminance(b)),
            luminance(a).min(luminance(b)),
        );
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn depth_follows_the_environment() {
        let d = Depth::detect;
        assert_eq!(d(false, Some("truecolor"), None), Depth::Truecolor);
        assert_eq!(d(false, Some("24bit"), Some("xterm")), Depth::Truecolor);
        assert_eq!(d(false, None, Some("xterm-256color")), Depth::Ansi256);
        assert_eq!(d(false, None, Some("xterm")), Depth::Ansi16);
        assert_eq!(d(false, None, Some("dumb")), Depth::None);
        assert_eq!(d(false, None, Some("")), Depth::None);
        assert_eq!(d(false, None, None), Depth::None);
        assert_eq!(
            d(true, Some("truecolor"), Some("xterm-256color")),
            Depth::None
        );
    }

    #[test]
    fn accents_need_a_color_terminal() {
        let a = Some(Color::Rgb(95, 215, 255));
        assert_eq!(accent(Theme::Dark, Depth::Truecolor), a);
        assert_eq!(accent(Theme::Dark, Depth::Ansi256), a);
        assert_eq!(accent(Theme::Dark, Depth::None), None);
    }

    #[test]
    fn downgrade_picks_the_nearest_palette_entry() {
        let red = Color::Rgb(255, 95, 95);
        assert_eq!(downgrade(Depth::Truecolor, red), red);
        assert_eq!(downgrade(Depth::Ansi256, red), Color::Indexed(203));
        assert_eq!(downgrade(Depth::Ansi16, red), Color::LightRed);
        let gray = Color::Rgb(128, 128, 128);
        assert_eq!(downgrade(Depth::Ansi256, gray), Color::Indexed(244));
        assert_eq!(downgrade(Depth::Ansi16, gray), Color::DarkGray);
        assert_eq!(downgrade(Depth::Ansi256, Color::Red), Color::Red);
    }

    #[test]
    fn accent_meets_wcag_aa_on_both_backgrounds() {
        assert!(contrast(Theme::Dark.accent(), BG) >= 4.5);
        assert_eq!(Theme::Light.accent(), Color::Rgb(0, 95, 135));
        assert!(contrast(Theme::Light.accent(), WHITE) >= 4.5);
    }

    #[test]
    fn ink_meets_wcag_aa_on_both_backgrounds() {
        for p in Paint::ALL {
            assert!(contrast(ink(Theme::Dark, p), BG) >= 4.5, "{p:?}");
            assert!(contrast(ink(Theme::Light, p), WHITE) >= 4.5, "{p:?}");
        }
        assert_eq!(ink(Theme::Light, Paint::Red), Color::Rgb(114, 42, 42));
    }

    #[test]
    fn default_text_meets_wcag_aa_on_every_fill() {
        for p in Paint::ALL {
            assert!(contrast(TEXT, fill(Theme::Dark, p)) >= 4.5, "{p:?}");
            assert!(contrast(DARK_TEXT, fill(Theme::Light, p)) >= 4.5, "{p:?}");
        }
    }

    #[test]
    fn fill_blends_ink_over_the_background() {
        assert_eq!(fill(Theme::Dark, Paint::Red), Color::Rgb(86, 46, 46));
        assert_eq!(fill(Theme::Light, Paint::Red), Color::Rgb(255, 215, 215));
    }

    #[test]
    fn osc11_replies_pick_a_theme() {
        let p = |s: &str| parse_osc11(s.as_bytes());
        assert_eq!(p("\x1b]11;rgb:ffff/ffff/ffff\x07"), Some(Theme::Light));
        assert_eq!(p("\x1b]11;rgb:0000/0000/0000\x1b\\"), Some(Theme::Dark));
        assert_eq!(p("\x1b]11;rgb:ff/ff/ff\x07"), Some(Theme::Light));
        assert_eq!(p("\x1b]11;rgb:1e/1e/1e\x07\x1b[?62;c"), Some(Theme::Dark));
        assert_eq!(p("\x1b[?62;c"), None);
        assert_eq!(p("\x1b]11;rgb:zz/ff/ff\x07"), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_reply_after_150ms_is_read_before_the_event_loop_can_see_it() {
        let mut fds = [0i32; 2];
        // SAFETY: `fds` holds the two ints `pipe` fills.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let w = fds[1];
        let writer = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            let reply = b"\x1b]11;rgb:ffff/ffff/ffff\x07\x1b[?62;c";
            // SAFETY: `reply` is readable for its length and `w` is the open write end.
            unsafe { libc::write(w, reply.as_ptr().cast(), reply.len()) };
        });
        let reply = read_reply(fds[0], REPLY_TIMEOUT);
        writer.join().unwrap();
        assert_eq!(parse_osc11(&reply), Some(Theme::Light));
        assert!(answered_da1(&reply));
    }

    #[test]
    fn auto_asks_the_terminal_only_when_colors_are_on() {
        let tc = Depth::Truecolor;
        let never = || -> Option<Theme> { panic!("must not query") };
        assert_eq!(resolve(ThemeSetting::Dark, tc, never), Theme::Dark);
        assert_eq!(resolve(ThemeSetting::Light, tc, never), Theme::Light);
        assert_eq!(resolve(ThemeSetting::Auto, Depth::None, never), Theme::Dark);
        assert_eq!(resolve(ThemeSetting::Auto, Depth::None, never), Theme::Dark);
        let light = || Some(Theme::Light);
        assert_eq!(resolve(ThemeSetting::Auto, tc, light), Theme::Light);
        assert_eq!(
            resolve(ThemeSetting::Auto, Depth::Ansi256, || None),
            Theme::Dark
        );
    }
}
