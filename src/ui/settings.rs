use super::actions::Action;
use super::list::{List, Row};
use super::theme::ThemeSetting;
use crate::config::{CommandUi, Config};
use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    AutosaveMs,
    DoubleClickMs,
    Grid,
    DefaultCanvas,
    HistoryLimit,
    ExportPadding,
    Motion,
    QuickConnect,
    QuickFixes,
    CommandUi,
    ScrollSpeed,
    Theme,
    ChromeAccent,
    ObsidianVault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Flag,
    Choice(&'static [&'static str]),
    Number,
    Text,
}

impl Setting {
    pub const ALL: [Setting; 14] = [
        Setting::AutosaveMs,
        Setting::DoubleClickMs,
        Setting::Grid,
        Setting::DefaultCanvas,
        Setting::HistoryLimit,
        Setting::ExportPadding,
        Setting::Motion,
        Setting::QuickConnect,
        Setting::QuickFixes,
        Setting::CommandUi,
        Setting::ScrollSpeed,
        Setting::Theme,
        Setting::ChromeAccent,
        Setting::ObsidianVault,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Setting::AutosaveMs => "autosave_ms",
            Setting::DoubleClickMs => "double_click_ms",
            Setting::Grid => "grid",
            Setting::DefaultCanvas => "default_canvas",
            Setting::HistoryLimit => "history_limit",
            Setting::ExportPadding => "export_padding",
            Setting::Motion => "motion",
            Setting::QuickConnect => "quick_connect",
            Setting::QuickFixes => "quick_fixes",
            Setting::CommandUi => "command_ui",
            Setting::ScrollSpeed => "scroll_speed",
            Setting::Theme => "theme",
            Setting::ChromeAccent => "chrome_accent",
            Setting::ObsidianVault => "obsidian_vault",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Setting::Motion | Setting::QuickConnect | Setting::QuickFixes => Kind::Flag,
            Setting::CommandUi => Kind::Choice(&["palette", "vim"]),
            Setting::Theme => Kind::Choice(&["auto", "dark", "light"]),
            Setting::AutosaveMs
            | Setting::DoubleClickMs
            | Setting::Grid
            | Setting::HistoryLimit
            | Setting::ExportPadding
            | Setting::ScrollSpeed => Kind::Number,
            Setting::DefaultCanvas | Setting::ChromeAccent | Setting::ObsidianVault => Kind::Text,
        }
    }

    /// The value as the user types it; `unset` for an absent optional key.
    pub fn show(self, c: &Config) -> String {
        match self {
            Setting::AutosaveMs => c.autosave.as_millis().to_string(),
            Setting::DoubleClickMs => c.double_click.as_millis().to_string(),
            Setting::Grid => c.grid.to_string(),
            Setting::DefaultCanvas => c.default_canvas.clone(),
            Setting::HistoryLimit => c.history_limit.to_string(),
            Setting::ExportPadding => c.export_padding.to_string(),
            Setting::Motion => c.motion.to_string(),
            Setting::QuickConnect => c.quick_connect.to_string(),
            Setting::QuickFixes => c.quick_fixes.to_string(),
            Setting::CommandUi => match c.command_ui {
                CommandUi::Palette => "palette",
                CommandUi::Vim => "vim",
            }
            .into(),
            Setting::ScrollSpeed => c.scroll_speed.to_string(),
            Setting::Theme => match c.theme {
                ThemeSetting::Auto => "auto",
                ThemeSetting::Dark => "dark",
                ThemeSetting::Light => "light",
            }
            .into(),
            Setting::ChromeAccent => match c.chrome_accent {
                Some(Color::Rgb(r, g, b)) => format!("#{r:02x}{g:02x}{b:02x}"),
                _ => "unset".into(),
            },
            Setting::ObsidianVault => c
                .obsidian_vault
                .as_ref()
                .map_or("unset".into(), |p| p.display().to_string()),
        }
    }

    /// Typed text as TOML: text kinds need quotes and escapes.
    pub fn toml(self, typed: &str) -> String {
        match self.kind() {
            // a JSON string is a valid TOML basic string; toml has no serializer here
            Kind::Text => serde_json::Value::from(typed.trim()).to_string(),
            _ => typed.trim().into(),
        }
    }
}

pub fn list(cfg: &Config, select: Setting) -> List<Action> {
    let rows = Setting::ALL
        .iter()
        .map(|&s| Row {
            value: Action::Setting(s),
            label: format!("{} = {}", s.key(), s.show(cfg)),
            key: match (s.kind(), s) {
                (Kind::Flag, _) => "toggle",
                (_, Setting::Theme) => "cycle, auto needs restart",
                (Kind::Choice(_), _) => "cycle",
                _ => "edit",
            }
            .into(),
        })
        .collect();
    let at = Setting::ALL.iter().position(|&s| s == select).unwrap_or(0);
    List::new("Settings", rows).min_width(52).select(at)
}
