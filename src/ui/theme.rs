use ratatui::style::Color;

pub const ACCENT: Color = Color::Rgb(95, 215, 255);

pub fn accent(no_color: bool, colorterm: Option<&str>) -> Option<Color> {
    let truecolor = matches!(colorterm, Some("truecolor" | "24bit"));
    (!no_color && truecolor).then_some(ACCENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BACKGROUND: Color = Color::Rgb(30, 30, 30);

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
        assert!(contrast(ACCENT, BACKGROUND) >= 4.5);
    }
}
