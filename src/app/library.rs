use super::App;
use crate::ui::Status;

impl App {
    pub(super) fn library(&mut self, arg: &str) {
        let (sub, name) = arg.split_once(char::is_whitespace).unwrap_or((arg, ""));
        let name = name.trim();
        let store = self.store.library();
        self.status = Some(match sub {
            "save" | "insert" if name.is_empty() => usage(),
            "save" if self.session.selection.is_empty() => {
                Status::error("Select shapes to save first")
            }
            "save" => {
                let snippet = self
                    .session
                    .canvas
                    .clone_canvas(&self.session.selection, 0, 0);
                match store.save(name, &snippet) {
                    Ok(_) => Status::notice(format!("Saved {name} to the library")),
                    Err(e) => failed(e),
                }
            }
            "insert" => match store.load(name) {
                Ok(snippet) => {
                    let ids: Vec<_> = snippet.elements.iter().map(|e| e.id().clone()).collect();
                    let (x, y) = snippet.origin();
                    let (vx, vy, ..) = self.session.viewport();
                    let (wx, wy) = self.session.view.to_world(vx + 2, vy + 1);
                    self.session
                        .insert(snippet.clone_canvas(&ids, wx - x, wy - y));
                    Status::notice(format!("Inserted {name}"))
                }
                Err(e) => failed(e),
            },
            _ => usage(),
        });
    }
}

fn usage() -> Status {
    Status::error("Usage: :lib save|insert <name>")
}

fn failed(e: crate::store::StoreError) -> Status {
    Status::error(format!("Library failed: {e}"))
}
