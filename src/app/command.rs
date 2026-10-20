use super::App;
use crate::ui::{Perf, Status};

impl App {
    pub(super) fn run_command(&mut self, line: &str) {
        let line = line.trim().trim_start_matches(':');
        if let Some(term) = line.strip_prefix('/') {
            if !self.session.find(term.trim()) {
                self.status = Some(Status::error("No match"));
            }
            return;
        }
        let (cmd, arg) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match cmd {
            "" => {}
            "q" => self.quit = true,
            "w" => self.save(),
            "wq" => {
                self.save();
                self.quit = true;
            }
            "e" if arg.trim().is_empty() => self.status = Some(Status::error("Usage: :e <name>")),
            "e" => self.switch_to(arg.trim()),
            "perf" => self.perf = self.perf.is_none().then(Perf::default),
            "u" => self.status = self.session.undo().map(Status::notice),
            "redo" => self.status = self.session.redo().map(Status::notice),
            _ => self.status = Some(Status::error(format!("Unknown command: {cmd}"))),
        }
    }
}
