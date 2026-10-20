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

pub fn colors(no_color: bool, colorterm: Option<&str>) -> bool {
    !no_color && matches!(colorterm, Some("truecolor" | "24bit"))
}

pub fn accent(theme: Theme, no_color: bool, colorterm: Option<&str>) -> Option<Color> {
    colors(no_color, colorterm).then(|| theme.accent())
}

/// Light ink is the dark ink scaled down: every paint then keeps 4.5:1 on white.
pub fn ink(theme: Theme, p: Paint) -> Color {
    let (r, g, b) = p.rgb();
    match theme {
        Theme::Dark => Color::Rgb(r, g, b),
        Theme::Light => Color::Rgb(darken(r), darken(g), darken(b)),
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
pub fn resolve(
    setting: ThemeSetting,
    no_color: bool,
    colorterm: Option<&str>,
    detected: impl FnOnce() -> Option<Theme>,
) -> Theme {
    match setting {
        ThemeSetting::Dark => Theme::Dark,
        ThemeSetting::Light => Theme::Light,
        ThemeSetting::Auto if colors(no_color, colorterm) => detected().unwrap_or(Theme::Dark),
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
    use std::time::{Duration, Instant};
    let mut out = std::io::stdout();
    out.write_all(b"\x1b]11;?\x07\x1b[c").ok()?;
    out.flush().ok()?;
    let deadline = Instant::now() + Duration::from_millis(100);
    let mut reply = Vec::new();
    while !answered_da1(&reply) {
        let left = deadline.checked_duration_since(Instant::now())?;
        let mut fd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `fd` is one valid pollfd and the count says 1.
        if unsafe { libc::poll(&mut fd, 1, left.as_millis() as i32 + 1) } <= 0 {
            break;
        }
        let mut chunk = [0u8; 128];
        // SAFETY: `chunk` is writable for its whole length.
        let n = unsafe { libc::read(0, chunk.as_mut_ptr().cast(), chunk.len()) };
        if n <= 0 {
            break;
        }
        reply.extend_from_slice(&chunk[..n as usize]);
    }
    parse_osc11(&reply)
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
    fn accents_need_truecolor_and_no_no_color() {
        let a = Some(Color::Rgb(95, 215, 255));
        assert_eq!(accent(Theme::Dark, false, Some("truecolor")), a);
        assert_eq!(accent(Theme::Dark, false, Some("24bit")), a);
        assert_eq!(accent(Theme::Dark, false, Some("256color")), None);
        assert_eq!(accent(Theme::Dark, false, None), None);
        assert_eq!(accent(Theme::Dark, true, Some("truecolor")), None);
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

    #[test]
    fn auto_asks_the_terminal_only_when_colors_are_on() {
        let tc = Some("truecolor");
        let never = || -> Option<Theme> { panic!("must not query") };
        assert_eq!(resolve(ThemeSetting::Dark, false, tc, never), Theme::Dark);
        assert_eq!(resolve(ThemeSetting::Light, false, tc, never), Theme::Light);
        assert_eq!(resolve(ThemeSetting::Auto, true, tc, never), Theme::Dark);
        assert_eq!(resolve(ThemeSetting::Auto, false, None, never), Theme::Dark);
        let light = || Some(Theme::Light);
        assert_eq!(resolve(ThemeSetting::Auto, false, tc, light), Theme::Light);
        assert_eq!(resolve(ThemeSetting::Auto, false, tc, || None), Theme::Dark);
    }
}
