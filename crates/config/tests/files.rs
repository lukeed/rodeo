use std::ffi::OsString;
use std::path::{Path, PathBuf};

use config::{
    AgentPanelSort, Config, Edit, Migration, Paths, Preferences, ResetKeys, StatusIndicators,
    ToastDelivery, WriteError, reset_keys,
};

const SAMPLE: &str = r#"onboarding = false

[keys]
prefix = "ctrl+b"
previous_tab = "alt+["   # tabs
next_tab = "alt+]"

[ui.sound]
enabled = true

[ui.toast]
enabled = true

[ui]
agent_panel_sort = "spaces"

[theme]
# name = "rose-pine"
name = "tokyo-night"
# name = "vesper"

[terminal]
new_cwd = "follow"
"#;

fn apply(edit: &Edit, text: &str) -> String {
    let updated = edit.apply(text).unwrap();
    let loaded = Config::parse(&updated, Path::new("/"));
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    updated
}

#[test]
fn theme_edit_keeps_the_file() {
    let updated = apply(&Edit::Theme("vesper".into()), SAMPLE);
    let expected = SAMPLE.replace(
        "name = \"tokyo-night\"\n# name = \"vesper\"\n",
        "name = \"vesper\"\nauto_switch = false\n# name = \"vesper\"\n",
    );
    assert_eq!(updated, expected);
    let again = apply(&Edit::Theme("nord".into()), &updated);
    assert_eq!(
        again,
        expected.replace("name = \"vesper\"\nauto", "name = \"nord\"\nauto")
    );
}

#[test]
fn ui_edits_keep_the_file() {
    let indicators = apply(&Edit::StatusIndicators(StatusIndicators::Symbols), SAMPLE);
    assert_eq!(
        indicators,
        SAMPLE.replace(
            "agent_panel_sort = \"spaces\"\n",
            "agent_panel_sort = \"spaces\"\nstatus_indicators = \"symbols\"\n"
        )
    );
    let sound = apply(&Edit::Sound(false), SAMPLE);
    assert_eq!(
        sound,
        SAMPLE.replace("[ui.sound]\nenabled = true", "[ui.sound]\nenabled = false")
    );
    let toast = apply(&Edit::ToastDelivery(ToastDelivery::System), SAMPLE);
    assert_eq!(
        toast,
        SAMPLE.replace(
            "[ui.toast]\nenabled = true",
            "[ui.toast]\ndelivery = \"system\""
        )
    );
    let config = Config::parse(&toast, Path::new("/")).config;
    assert_eq!(config.ui.toast.delivery, ToastDelivery::System);
}

#[test]
fn edits_create_missing_sections() {
    assert_eq!(
        apply(&Edit::Theme("dracula".into()), ""),
        "[theme]\nname = \"dracula\"\nauto_switch = false\n"
    );
    assert_eq!(
        apply(&Edit::ToastDelivery(ToastDelivery::Herdr), "# mine\n"),
        "# mine\n[ui.toast]\ndelivery = \"herdr\"\n"
    );
    assert_eq!(
        apply(&Edit::Sound(true), "[ui]\nsidebar_width = 30\n"),
        "[ui]\nsidebar_width = 30\n\n[ui.sound]\nenabled = true\n"
    );
    let dotted = apply(&Edit::Sound(false), "ui.sound.enabled = true\n");
    assert_eq!(dotted, "ui.sound.enabled = false\n");
    let inline = apply(
        &Edit::StatusIndicators(StatusIndicators::Dots),
        "ui = { sidebar_width = 30 }\n",
    );
    assert_eq!(
        Config::parse(&inline, Path::new("/"))
            .config
            .ui
            .sidebar_width,
        30
    );
    assert!(inline.contains("status_indicators = \"dots\""), "{inline}");
}

#[test]
fn edit_errors() {
    assert!(matches!(
        Edit::Sound(true).apply("[ui\n"),
        Err(WriteError::Toml(_))
    ));
    let shape = Edit::Sound(true).apply("ui = 3\n").unwrap_err();
    assert_eq!(
        shape.to_string(),
        "config.toml has `ui` set to a value that is not a table"
    );
    assert_eq!(Edit::Sound(true).description(), "sound setting");
}

#[test]
fn edits_write_files() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("nested/config.toml");
    Edit::Theme("nord".into()).write(&path).unwrap();
    Edit::ToastDelivery(ToastDelivery::Terminal)
        .write(&path)
        .unwrap();
    let loaded = Config::load(&path);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    assert_eq!(loaded.config.theme.name.as_deref(), Some("nord"));
    assert_eq!(loaded.config.ui.toast.delivery, ToastDelivery::Terminal);
    let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(leftovers, [OsString::from("config.toml")]);

    std::fs::write(&path, "\u{feff}[ui]\nsidebar_width = 30\n").unwrap();
    Edit::Sound(false).write(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        text,
        "[ui]\nsidebar_width = 30\n\n[ui.sound]\nenabled = false\n"
    );
}

#[test]
fn reset_keys_removes_key_tables() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.toml");
    assert_eq!(reset_keys(&path).unwrap(), ResetKeys::NoFile);
    std::fs::write(&path, "[ui]\nsidebar_width = 30\n").unwrap();
    assert_eq!(reset_keys(&path).unwrap(), ResetKeys::NoKeys);

    std::fs::write(
        &path,
        "onboarding = false\nkeys.help = \"prefix+h\"\nkeys.indexed.tabs = \"alt\"\n[ui]\nx = 1\n",
    )
    .unwrap();
    assert!(matches!(
        reset_keys(&path).unwrap(),
        ResetKeys::Reset { .. }
    ));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "onboarding = false\n[ui]\nx = 1\n"
    );

    let original = "onboarding = false\n\n# ui first\n[ui]\nsidebar_width = 30\n\n\
                    [keys]\nprefix = \"ctrl+a\"\n\n[keys.indexed]\ntabs = \"ctrl\"\n\n\
                    [[keys.command]]\ncommand = \"ls\"\nkey = \"prefix+alt+l\"\n\n\
                    [theme]\nname = \"nord\" # mine\n";
    std::fs::write(&path, original).unwrap();
    let ResetKeys::Reset { backup } = reset_keys(&path).unwrap() else {
        panic!("expected a reset");
    };
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
    assert!(
        backup
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("config.toml.bak-keys-")
    );
    let updated = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        updated,
        "onboarding = false\n\n# ui first\n[ui]\nsidebar_width = 30\n\n[theme]\nname = \"nord\" # mine\n"
    );
    let loaded = Config::load(&path);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    assert_eq!(loaded.config.keys, config::KeysConfig::default());

    std::fs::write(&path, "[keys]\nhelp = \"prefix+h\"\n").unwrap();
    let ResetKeys::Reset { backup: second } = reset_keys(&path).unwrap() else {
        panic!("expected a reset");
    };
    assert_ne!(second, backup);
    std::fs::write(&path, "[keys\n").unwrap();
    assert!(matches!(reset_keys(&path), Err(WriteError::Toml(_))));
}

#[test]
fn preferences_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state/preferences/default.json");
    assert_eq!(Preferences::load(&path), Preferences::default());
    let prefs = Preferences {
        sidebar_width: Some(31),
        sidebar_section_split: Some(0.25),
        sidebar_collapsed: Some(true),
        agent_panel_sort: Some(AgentPanelSort::Priority),
        collapsed_groups: vec!["/repo/.git".into()],
    };
    prefs.save(&path).unwrap();
    assert_eq!(Preferences::load(&path), prefs);
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(json["agent_panel_sort"], "priority");
    assert_eq!(json["sidebar_width"], 31);

    Preferences::default().save(&path).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");

    std::fs::write(&path, "not json").unwrap();
    assert_eq!(Preferences::load(&path), Preferences::default());
    std::fs::write(
        &path,
        "{\"sidebar_width\": 20, \"remote_collapsed_groups\": []}",
    )
    .unwrap();
    assert_eq!(Preferences::load(&path).sidebar_width, Some(20));
}

#[test]
fn section_split_bounds() {
    let split = |value: Option<f32>| {
        Preferences {
            sidebar_section_split: value,
            ..Preferences::default()
        }
        .section_split()
    };
    assert!((split(None) - 0.5).abs() < f32::EPSILON);
    assert!((split(Some(0.05)) - 0.1).abs() < f32::EPSILON);
    assert!((split(Some(0.95)) - 0.9).abs() < f32::EPSILON);
    assert!((split(Some(0.3)) - 0.3).abs() < f32::EPSILON);
    assert!((split(Some(f32::NAN)) - 0.5).abs() < f32::EPSILON);
}

fn env(pairs: &[(&str, &Path)]) -> impl Fn(&str) -> Option<OsString> {
    let pairs: Vec<(String, OsString)> = pairs
        .iter()
        .map(|(name, value)| ((*name).to_string(), value.as_os_str().to_owned()))
        .collect();
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }
}

#[test]
fn path_resolution() {
    let home = Path::new("/home/me");
    let paths = Paths::from_lookup(env(&[("HOME", home)]));
    assert_eq!(
        paths.config,
        PathBuf::from("/home/me/.config/drover/config.toml")
    );
    assert_eq!(paths.state, PathBuf::from("/home/me/.local/state/drover"));
    assert_eq!(
        paths.herdr_config,
        PathBuf::from("/home/me/.config/herdr/config.toml")
    );
    assert_eq!(paths.config_dir(), Path::new("/home/me/.config/drover"));

    let xdg = Paths::from_lookup(env(&[
        ("HOME", home),
        ("XDG_CONFIG_HOME", Path::new("/xdg/config")),
        ("XDG_STATE_HOME", Path::new("/xdg/state")),
    ]));
    assert_eq!(xdg.config, PathBuf::from("/xdg/config/drover/config.toml"));
    assert_eq!(xdg.state, PathBuf::from("/xdg/state/drover"));
    assert_eq!(
        xdg.herdr_config,
        PathBuf::from("/xdg/config/herdr/config.toml")
    );

    let explicit = Paths::from_lookup(env(&[
        ("HOME", home),
        ("DROVER_CONFIG_PATH", Path::new("/etc/drover.toml")),
        ("HERDR_CONFIG_PATH", Path::new("/etc/herdr.toml")),
        ("XDG_CONFIG_HOME", Path::new("")),
    ]));
    assert_eq!(explicit.config, PathBuf::from("/etc/drover.toml"));
    assert_eq!(explicit.herdr_config, PathBuf::from("/etc/herdr.toml"));

    let homeless = Paths::from_lookup(env(&[]));
    assert!(homeless.config.starts_with(std::env::temp_dir()));
    assert!(homeless.state.starts_with(std::env::temp_dir()));

    assert_eq!(
        paths.preferences("work"),
        PathBuf::from("/home/me/.local/state/drover/preferences/work.json")
    );
    assert_eq!(
        paths.preferences("a/b c"),
        PathBuf::from("/home/me/.local/state/drover/preferences/a_b_c.json")
    );
    assert_eq!(
        paths.preferences(".."),
        PathBuf::from("/home/me/.local/state/drover/preferences/default.json")
    );
    assert_eq!(
        paths.preferences(""),
        PathBuf::from("/home/me/.local/state/drover/preferences/default.json")
    );
}

#[test]
fn migration_copies_herdr_config_once() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    let paths = Paths::from_lookup(env(&[("HOME", home)]));
    assert_eq!(paths.migrate().unwrap(), None);
    assert!(!paths.config.exists());

    let herdr = home.join(".config/herdr/config.toml");
    std::fs::create_dir_all(herdr.parent().unwrap()).unwrap();
    std::fs::write(&herdr, "[theme]\nname = \"nord\"\n").unwrap();
    assert_eq!(
        paths.migrate().unwrap(),
        Some(Migration {
            from: herdr.clone(),
            to: home.join(".config/drover/config.toml"),
        })
    );
    assert_eq!(
        std::fs::read_to_string(&paths.config).unwrap(),
        "[theme]\nname = \"nord\"\n"
    );

    std::fs::write(&herdr, "[theme]\nname = \"vesper\"\n").unwrap();
    assert_eq!(paths.migrate().unwrap(), None);
    assert_eq!(
        std::fs::read_to_string(&paths.config).unwrap(),
        "[theme]\nname = \"nord\"\n"
    );

    Edit::Theme("dracula".into()).write(&paths.config).unwrap();
    assert_eq!(
        std::fs::read_to_string(&herdr).unwrap(),
        "[theme]\nname = \"vesper\"\n"
    );
}

#[test]
fn migration_uses_herdr_overrides() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let herdr = root.join("custom/herdr.toml");
    std::fs::create_dir_all(herdr.parent().unwrap()).unwrap();
    std::fs::write(&herdr, "onboarding = false\n").unwrap();
    let xdg = root.join("xdg");
    let paths = Paths::from_lookup(env(&[
        ("HOME", root),
        ("XDG_CONFIG_HOME", &xdg),
        ("HERDR_CONFIG_PATH", &herdr),
    ]));
    let migration = paths.migrate().unwrap().unwrap();
    assert_eq!(migration.to, xdg.join("drover/config.toml"));
    assert_eq!(
        std::fs::read_to_string(&migration.to).unwrap(),
        "onboarding = false\n"
    );

    let directory = root.join("dir-herdr");
    std::fs::create_dir_all(&directory).unwrap();
    let other = Paths::from_lookup(env(&[
        ("HOME", root),
        ("DROVER_CONFIG_PATH", &root.join("second.toml")),
        ("HERDR_CONFIG_PATH", &directory),
    ]));
    assert_eq!(other.migrate().unwrap(), None);
}
