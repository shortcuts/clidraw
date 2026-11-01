/// Canvas conventions for agents. Served as MCP `instructions` and reused by `clidraw setup`.
pub const CONVENTIONS: &str = r#"clidraw canvases outlive the agent that draws them. Draw so a later agent with no context can read and extend them.

Before you draw
- Call `search` with the topic. Call `show` on each canvas that matches.
- A canvas for the topic exists: extend it with `edit`. Never create a second one.
- No canvas exists: call `new`.

Canvas name
- Use `<repo>-<topic>` in lowercase kebab-case, such as `clidraw-auth-flow`. `<repo>` is the repository directory name.
- Name the topic. Do not use a date or a task name.

Shapes and lines
- Give every shape an `id` that names the thing: a short kind prefix, then the name, such as `svc-auth` or `db-users`.
- Give every line the id `<from>-to-<to>`, such as `svc-auth-to-db-users`.
- Never leave an id random. A later agent edits an element by its id.
- Give every shape a short `label` that a person reads. Label a line when its meaning is not obvious.
- Join shapes with `connect` and their ids, so the line follows the shapes.
- After you add shapes, call `layout`. Use `dir` `td` by default and `lr` for a flow over time.

Code and facts
- Point a shape at its code with `set` and `link`: `path` or `path:line`, relative to the repository root. `add_shape` has no `link` field, so call `set` after it.
- Put other facts a reader needs in `meta`, such as the owner or the data format.
"#;
