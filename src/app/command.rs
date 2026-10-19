use super::App;

impl App {
    pub(super) fn run_command(&mut self, line: &str) {
        let line = line.trim().trim_start_matches(':');
        let (cmd, arg) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match cmd {
            "" => {}
            "q" => self.quit = true,
            "w" => self.save(),
            "wq" => {
                self.save();
                self.quit = true;
            }
            "e" if arg.trim().is_empty() => self.status = Some("Usage: :e <name>".into()),
            "e" => self.switch_to(arg.trim()),
            "u" => self.session.undo(),
            "redo" => self.session.redo(),
            _ => self.status = Some(format!("Unknown command: {cmd}")),
        }
    }
}
