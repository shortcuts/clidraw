use super::App;
use crate::config::Config;
use crate::ui::prompt::{Prompt, PromptKind};
use crate::ui::settings::{self, Kind, Setting};
use crate::ui::theme::ThemeSetting;
use crate::ui::{Overlay, Status};

impl App {
    pub(super) fn open_settings(&mut self, select: Setting) {
        let (cfg, _) = Config::load(self.store.dir());
        self.overlay = Overlay::Actions(settings::list(&cfg, select));
    }

    pub(super) fn setting_action(&mut self, s: Setting) {
        let (cfg, _) = Config::load(self.store.dir());
        let shown = s.show(&cfg);
        match s.kind() {
            Kind::Flag => self.save_setting(s, (shown != "true").to_string()),
            Kind::Choice(options) => {
                let next = options
                    .iter()
                    .position(|o| *o == shown)
                    .map_or(0, |i| i + 1);
                self.save_setting(s, format!("\"{}\"", options[next % options.len()]));
            }
            Kind::Number | Kind::Text => {
                let text = if shown == "unset" {
                    String::new()
                } else {
                    shown
                };
                self.overlay = Overlay::Prompt(Prompt::new(PromptKind::Setting(s), text));
            }
        }
    }

    pub(super) fn save_setting(&mut self, s: Setting, toml: String) {
        match Config::save(self.store.dir(), s.key(), &toml) {
            Ok(()) => {
                let (cfg, _) = Config::load(self.store.dir());
                self.apply_config(&cfg);
                // detect needs stdin, which the event loop owns after startup
                if s == Setting::Theme && cfg.theme == ThemeSetting::Auto {
                    self.status = Some(Status::notice(
                        "Restart clidraw to detect the terminal theme",
                    ));
                }
            }
            Err(e) => self.status = Some(Status::error(format!("{}: {e}", s.key()))),
        }
        self.open_settings(s);
    }

    pub(super) fn reset_settings(&mut self) {
        let (old, _) = Config::load(self.store.dir());
        match Config::reset(self.store.dir()) {
            Ok(()) => {
                let (cfg, _) = Config::load(self.store.dir());
                self.apply_config(&cfg);
                self.status = Some(if old.theme == ThemeSetting::Auto {
                    Status::notice("Settings reset to defaults")
                } else {
                    Status::notice(
                        "Settings reset to defaults. Restart clidraw to detect the terminal theme",
                    )
                });
            }
            Err(e) => self.status = Some(Status::error(format!("config.toml: {e}"))),
        }
    }
}
