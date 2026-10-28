use clidraw::config::{CommandUi, Config, DEFAULT_TOML};
use clidraw::ui::theme::ThemeSetting;
use std::time::Duration;

fn load_text(text: &str) -> (Config, Vec<String>, tempfile::TempDir) {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("config.toml"), text).unwrap();
    let (c, w) = Config::load(t.path());
    (c, w, t)
}

#[test]
fn missing_file_gives_defaults_and_writes_commented_file() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("new");
    let (c, w) = Config::load(&dir);
    assert_eq!(c, Config::default());
    assert!(w.is_empty());
    let file = dir.join("config.toml");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), DEFAULT_TOML);

    std::fs::write(&file, "autosave_ms = 900\n").unwrap();
    let (c, _) = Config::load(&dir);
    assert_eq!(c.autosave, Duration::from_millis(900));
    assert_eq!(std::fs::read(&file).unwrap(), b"autosave_ms = 900\n");
}

#[test]
fn overrides_apply() {
    let (c, w, _t) = load_text(
        "autosave_ms = 100\ndouble_click_ms = 250\ndefault_canvas = \"scratch\"\nhistory_limit = 50\n",
    );
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(c.autosave, Duration::from_millis(100));
    assert_eq!(c.double_click, Duration::from_millis(250));
    assert_eq!(c.default_canvas, "scratch");
    assert_eq!(c.history_limit, 50);
}

#[test]
fn default_text_matches_defaults() {
    let live: String = DEFAULT_TOML
        .lines()
        // Unset means the theme accent, which no single "#rrggbb" line can state.
        .filter(|l| !l.contains("chrome_accent ="))
        // The example folder does not exist on the test machine.
        .filter(|l| !l.contains("obsidian_vault ="))
        .map(|l| match l.strip_prefix("# ") {
            Some(rest) if rest.contains(" = ") => format!("{rest}\n"),
            _ => format!("{l}\n"),
        })
        .collect();
    assert!(live.contains("autosave_ms = 500"), "{live}");
    let (c, w, _t) = load_text(&live);
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(c, Config::default());
}

#[test]
fn unknown_key_warns_and_rest_applies() {
    let (c, w, _t) = load_text("bogus = 1\nautosave_ms = 100\n");
    assert_eq!(c.autosave, Duration::from_millis(100));
    assert_eq!(w, vec!["config.toml: unknown key bogus"]);
}

#[test]
fn wrong_type_keeps_default_for_that_key() {
    let (c, w, _t) = load_text("autosave_ms = \"fast\"\ndouble_click_ms = 250\n");
    assert_eq!(c.autosave, Config::default().autosave);
    assert_eq!(c.double_click, Duration::from_millis(250));
    assert_eq!(w.len(), 1);
    assert!(w[0].starts_with("config.toml: key autosave_ms: "), "{w:?}");
}

#[test]
fn dock_key_is_unknown_and_warns() {
    let (_c, w, _t) = load_text("dock = \"left\"\n");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("dock"), "{w:?}");
}

#[test]
fn zero_autosave_warns() {
    let (c, w, _t) = load_text("autosave_ms = 0\n");
    assert_eq!(c.autosave, Config::default().autosave);
    assert!(w[0].starts_with("config.toml: key autosave_ms: "), "{w:?}");
}

#[test]
fn invalid_canvas_name_warns() {
    let (c, w, _t) = load_text("default_canvas = \"a/b\"\n");
    assert_eq!(c.default_canvas, "untitled");
    assert!(
        w[0].starts_with("config.toml: key default_canvas: "),
        "{w:?}"
    );
}

#[test]
fn syntax_error_gives_defaults_and_names_the_line() {
    let (c, w, _t) = load_text("autosave_ms = 100\ndock = \n");
    assert_eq!(c, Config::default());
    assert_eq!(w.len(), 1);
    assert!(w[0].starts_with("config.toml: "), "{w:?}");
    assert!(w[0].contains("line 2"), "{w:?}");
    assert!(!w[0].contains('\n'), "{w:?}");
}

#[cfg(unix)]
#[test]
fn symlinked_dir_is_followed() {
    let t = tempfile::tempdir().unwrap();
    let real = t.path().join("dotfiles");
    std::fs::create_dir(&real).unwrap();
    let link = t.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    Config::load(&link);
    assert!(real.join("config.toml").exists());
    assert!(std::fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn zero_history_limit_warns() {
    let (c, w, _t) = load_text("history_limit = 0\n");
    assert_eq!(c.history_limit, Config::default().history_limit);
    assert_eq!(
        w,
        vec!["config.toml: key history_limit: expected a positive integer"]
    );
}

#[test]
fn grid_defaults_off_and_rejects_out_of_range() {
    assert_eq!(Config::default().grid, 1);
    let (c, w, _t) = load_text("grid = 4\n");
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(c.grid, 4);
    let (c, w, _t) = load_text("grid = 0\n");
    assert_eq!(c.grid, 1);
    assert_eq!(
        w,
        vec!["config.toml: key grid: expected an integer from 1 to 1000"]
    );
}

#[test]
fn export_padding_defaults_to_two_and_rejects_negative() {
    assert_eq!(Config::default().export_padding, 2);
    let (c, w, _t) = load_text("export_padding = 0\n");
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(c.export_padding, 0);
    let (c, w, _t) = load_text("export_padding = -1\n");
    assert_eq!(c.export_padding, 2);
    assert_eq!(
        w,
        vec!["config.toml: key export_padding: expected an integer from 0 to 100"]
    );
}

#[test]
fn motion_defaults_on_and_rejects_non_booleans() {
    assert!(Config::default().motion);
    let (c, w, _t) = load_text("motion = false\n");
    assert!(w.is_empty(), "{w:?}");
    assert!(!c.motion);
    let (c, w, _t) = load_text("motion = 3\n");
    assert!(c.motion);
    assert_eq!(w, vec!["config.toml: key motion: expected true or false"]);
}

#[test]
fn scroll_speed_defaults_to_100_and_rejects_out_of_range() {
    assert_eq!(Config::default().scroll_speed, 100);
    let (c, w, _t) = load_text("scroll_speed = 50\n");
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(c.scroll_speed, 50);
    let (c, w, _t) = load_text("scroll_speed = 0\n");
    assert_eq!(c.scroll_speed, 100);
    assert_eq!(
        w,
        vec!["config.toml: key scroll_speed: expected an integer from 1 to 1000"]
    );
}

#[test]
fn theme_defaults_to_auto_and_rejects_unknown_names() {
    assert_eq!(Config::default().theme, ThemeSetting::Auto);
    let (c, w, _t) = load_text("theme = \"light\"\n");
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(c.theme, ThemeSetting::Light);
    let (c, _, _t) = load_text("theme = \"Dark\"\n");
    assert_eq!(c.theme, ThemeSetting::Dark);
    let (c, w, _t) = load_text("theme = \"blue\"\n");
    assert_eq!(c.theme, ThemeSetting::Auto);
    assert_eq!(
        w,
        vec!["config.toml: key theme: expected \"auto\", \"dark\" or \"light\""]
    );
}

#[test]
fn chrome_accent_reads_a_hex_color_and_rejects_the_rest() {
    assert_eq!(Config::default().chrome_accent, None);
    let (c, w, _t) = load_text("chrome_accent = \"#ff8800\"\n");
    assert!(w.is_empty());
    assert_eq!(
        c.chrome_accent,
        Some(ratatui::style::Color::Rgb(255, 136, 0))
    );
    for bad in ["\"orange\"", "\"#ff88\"", "\"#gggggg\"", "5"] {
        let (c, w, _t) = load_text(&format!("chrome_accent = {bad}\n"));
        assert_eq!(c.chrome_accent, None);
        assert_eq!(
            w,
            vec!["config.toml: key chrome_accent: expected a \"#rrggbb\" string"]
        );
    }
}

#[test]
fn quick_fixes_defaults_on_and_accepts_false() {
    assert!(Config::default().quick_fixes);
    let (c, w, _t) = load_text("quick_fixes = false\n");
    assert!(!c.quick_fixes && w.is_empty());
    let (c, w, _t) = load_text("quick_fixes = \"yes\"\n");
    assert!(c.quick_fixes && w.len() == 1, "{w:?}");
}

#[test]
fn command_ui_defaults_to_palette_and_accepts_vim() {
    assert_eq!(Config::default().command_ui, CommandUi::Palette);
    let (c, w, _t) = load_text("command_ui = \"vim\"\n");
    assert!(c.command_ui == CommandUi::Vim && w.is_empty(), "{w:?}");
    let (c, w, _t) = load_text("command_ui = \"Vim\"\n");
    assert!(c.command_ui == CommandUi::Vim && w.is_empty(), "{w:?}");
    let (c, w, _t) = load_text("command_ui = \"emacs\"\n");
    assert!(c.command_ui == CommandUi::Palette && w.len() == 1, "{w:?}");
}

#[test]
fn obsidian_vault_defaults_to_none_and_needs_an_existing_folder() {
    assert_eq!(Config::default().obsidian_vault, None);
    let vault = tempfile::tempdir().unwrap();
    let path = vault.path().to_str().unwrap().replace('\\', "/");
    let (c, w, _t) = load_text(&format!("obsidian_vault = \"{path}\"\n"));
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(c.obsidian_vault, Some(vault.path().to_path_buf()));
    let (c, w, _t) = load_text("obsidian_vault = \"/missing/vault\"\n");
    assert_eq!(c.obsidian_vault, None);
    assert_eq!(
        w,
        vec!["config.toml: key obsidian_vault: expected a path to an existing folder"]
    );
    let (c, w, _t) = load_text("obsidian_vault = 3\n");
    assert_eq!(c.obsidian_vault, None);
    assert_eq!(w.len(), 1, "{w:?}");
}

#[test]
fn quick_connect_defaults_on_and_accepts_false() {
    assert!(Config::default().quick_connect);
    let (c, w, _t) = load_text("quick_connect = false\n");
    assert!(!c.quick_connect && w.is_empty());
    let (c, w, _t) = load_text("quick_connect = \"yes\"\n");
    assert!(c.quick_connect && w.len() == 1, "{w:?}");
}
