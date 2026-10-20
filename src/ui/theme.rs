use crate::model::{Paint, FILL_ALPHA};
use ratatui::style::Color;

pub const ACCENT: Color = Color::Rgb(95, 215, 255);
pub const BACKGROUND: (u8, u8, u8) = (30, 30, 30);

pub fn colors(no_color: bool, colorterm: Option<&str>) -> bool {
    !no_color && matches!(colorterm, Some("truecolor" | "24bit"))
}

pub fn accent(no_color: bool, colorterm: Option<&str>) -> Option<Color> {
    colors(no_color, colorterm).then_some(ACCENT)
}

pub fn ink(p: Paint) -> Color {
    let (r, g, b) = p.rgb();
    Color::Rgb(r, g, b)
}

/// The ink blended over the background: a terminal cell has no alpha.
pub fn fill(p: Paint) -> Color {
    let (r, g, b) = p.rgb();
    let mix = |c: u8, bg: u8| {
        ((u16::from(c) * u16::from(FILL_ALPHA) + u16::from(bg) * (100 - u16::from(FILL_ALPHA)))
            / 100) as u8
    };
    Color::Rgb(
        mix(r, BACKGROUND.0),
        mix(g, BACKGROUND.1),
        mix(b, BACKGROUND.2),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: Color = Color::Rgb(BACKGROUND.0, BACKGROUND.1, BACKGROUND.2);
    const TEXT: Color = Color::Rgb(229, 229, 229);

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
        assert_eq!(accent(false, Some("truecolor")), Some(ACCENT));
        assert_eq!(accent(false, Some("24bit")), Some(ACCENT));
        assert_eq!(accent(false, Some("256color")), None);
        assert_eq!(accent(false, None), None);
        assert_eq!(accent(true, Some("truecolor")), None);
    }

    #[test]
    fn accent_meets_wcag_aa_on_the_dark_background() {
        assert!(contrast(ACCENT, BG) >= 4.5);
    }

    #[test]
    fn ink_meets_wcag_aa_on_the_dark_background() {
        for p in Paint::ALL {
            assert!(contrast(ink(p), BG) >= 4.5, "{p:?}");
        }
    }

    #[test]
    fn default_text_meets_wcag_aa_on_every_fill() {
        for p in Paint::ALL {
            assert!(contrast(TEXT, fill(p)) >= 4.5, "{p:?}");
        }
    }

    #[test]
    fn fill_blends_ink_over_the_background() {
        assert_eq!(fill(Paint::Red), Color::Rgb(86, 46, 46));
    }
}
