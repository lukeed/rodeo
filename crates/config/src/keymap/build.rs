use std::collections::{BTreeMap, HashMap};

use crossterm::event::{KeyCode, KeyModifiers};

use super::action::{ACTION_FIELDS, NAVIGATE_FIELDS, Slot};
use super::combo::parse_modifiers;
use super::{Binding, CustomCommand, DEFAULT_PREFIX_COMBO, Indexed, KeyCombo, Keymap, Trigger};
use crate::keys::{self, CommandKind, KeysConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapBuild {
    pub keymap: Keymap,
    pub diagnostics: Vec<String>,
    pub prefix_error: Option<String>,
}

impl KeymapBuild {
    pub fn all_diagnostics(&self) -> Vec<String> {
        self.prefix_error
            .iter()
            .chain(&self.diagnostics)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    User,
    Default,
}

struct Owner {
    field: String,
    source: Source,
}

struct Registry {
    prefixes: Vec<KeyCombo>,
    prefix_source: Source,
    direct: HashMap<KeyCombo, Owner>,
    prefixed: HashMap<KeyCombo, Owner>,
}

impl Registry {
    fn new(prefixes: &[KeyCombo], prefix_source: Source) -> Self {
        let mut registry = Self {
            prefixes: prefixes.iter().map(|combo| combo.normalized()).collect(),
            prefix_source,
            direct: HashMap::new(),
            prefixed: HashMap::new(),
        };
        for combo in registry.prefixes.clone() {
            registry.reserve(combo, "keys.prefix", prefix_source);
        }
        registry
    }

    fn reserve(&mut self, combo: KeyCombo, field: &str, source: Source) {
        self.direct.entry(combo.normalized()).or_insert(Owner {
            field: field.into(),
            source,
        });
    }

    fn reserve_navigate_keys(&mut self) {
        let fixed = [
            KeyCode::Esc,
            KeyCode::Enter,
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Left,
            KeyCode::Right,
        ]
        .into_iter()
        .chain(('1'..='9').map(KeyCode::Char));
        for code in fixed {
            self.reserve(
                KeyCombo::plain(code),
                "navigate reserved keys",
                Source::Default,
            );
        }
    }

    fn owner(&self, binding: &Binding) -> Option<&Owner> {
        let combo = binding.combo.normalized();
        match binding.trigger {
            Trigger::Direct => self.direct.get(&combo),
            Trigger::Prefix => self.prefixed.get(&combo),
        }
    }

    fn register(&mut self, binding: &Binding, field: &str, source: Source) {
        let owner = Owner {
            field: field.into(),
            source,
        };
        let combo = binding.combo.normalized();
        match binding.trigger {
            Trigger::Direct => self.direct.insert(combo, owner),
            Trigger::Prefix => self.prefixed.insert(combo, owner),
        };
    }
}

enum Parsed {
    Single(Binding),
    Range(Vec<Binding>),
}

fn parse_binding(raw: &str) -> Option<Parsed> {
    let raw = raw.trim();
    let (trigger, body) = match raw.strip_prefix("prefix+") {
        Some(body) => (Trigger::Prefix, body),
        None => (Trigger::Direct, raw),
    };
    if let Some(modifiers) = range_modifiers(body) {
        let bindings = ('1'..='9')
            .map(|digit| Binding::new(trigger, KeyCombo::new(KeyCode::Char(digit), modifiers)))
            .collect();
        return Some(Parsed::Range(bindings));
    }
    KeyCombo::parse(body).map(|combo| Parsed::Single(Binding::new(trigger, combo)))
}

fn range_modifiers(body: &str) -> Option<KeyModifiers> {
    let mut modifiers = KeyModifiers::NONE;
    let mut ranged = false;
    for token in body.split('+').map(str::trim) {
        if token == "1..9" {
            if ranged {
                return None;
            }
            ranged = true;
        } else {
            modifiers |= super::combo::modifier(token)?;
        }
    }
    ranged.then_some(modifiers)
}

struct Builder<'a> {
    keys: &'a KeysConfig,
    main: Registry,
    navigate: Registry,
    diagnostics: Vec<String>,
}

impl Builder<'_> {
    fn note(&mut self, diagnostic: String) {
        self.diagnostics.push(diagnostic);
    }

    fn source(&self, field: &str) -> Source {
        if self.keys.binding(field).is_some() {
            Source::User
        } else {
            Source::Default
        }
    }

    fn values(&self, field: &str, default: &str) -> Vec<String> {
        match self.keys.binding(field) {
            Some(binding) => binding.values().to_vec(),
            None => vec![default.to_string()],
        }
    }

    fn parse_values(&mut self, field: &str, values: &[String]) -> Vec<(String, Parsed)> {
        let mut parsed = Vec::new();
        for raw in values.iter().map(|raw| raw.trim()) {
            if raw.is_empty() {
                continue;
            }
            match parse_binding(raw) {
                Some(binding) => parsed.push((raw.to_string(), binding)),
                None => self.note(format!(
                    "invalid keybinding: {field} = {raw:?}; disabling binding"
                )),
            }
        }
        parsed
    }

    fn action(&mut self, field: &str, values: &[String], source: Source) -> Vec<Binding> {
        self.singles(field, values, source, Self::accept)
    }

    fn navigate(&mut self, field: &str, values: &[String], source: Source) -> Vec<Binding> {
        self.singles(field, values, source, Self::accept_navigate)
    }

    fn singles(
        &mut self,
        field: &str,
        values: &[String],
        source: Source,
        accept: fn(&mut Self, &str, &Binding, Source) -> bool,
    ) -> Vec<Binding> {
        let mut bindings = Vec::new();
        for (raw, parsed) in self.parse_values(field, values) {
            match parsed {
                Parsed::Single(binding) => {
                    if accept(self, field, &binding, source) {
                        bindings.push(binding);
                    }
                }
                Parsed::Range(_) => self.note(format!(
                    "range keybinding is only valid for indexed actions: {field} = {raw:?}; disabling binding"
                )),
            }
        }
        bindings
    }

    fn indexed(&mut self, field: &str, values: &[String], source: Source) -> Vec<Binding> {
        let mut bindings = Vec::new();
        for (_, parsed) in self.parse_values(field, values) {
            let candidates = match parsed {
                Parsed::Single(binding) => vec![binding],
                Parsed::Range(range) => range,
            };
            for binding in candidates {
                if binding.combo.digit_index().is_none() {
                    self.note(format!(
                        "indexed keybinding must use 1..9: {field} = {:?}; disabling binding",
                        binding.label
                    ));
                } else if self.accept(field, &binding, source) {
                    bindings.push(binding);
                }
            }
        }
        bindings
    }

    fn legacy_indexed(&mut self, field: &str, value: &str, source: Source) -> Vec<Binding> {
        if value.trim().is_empty() {
            return Vec::new();
        }
        let Some(modifiers) = parse_modifiers(value) else {
            self.note(format!(
                "invalid indexed keybinding: {field} = {value:?}; disabling binding"
            ));
            return Vec::new();
        };
        let mut bindings = Vec::new();
        for digit in '1'..='9' {
            let binding = Binding {
                trigger: Trigger::Direct,
                combo: KeyCombo::new(KeyCode::Char(digit), modifiers),
                label: format!("{}+{digit}", value.trim()),
            };
            if self.accept(field, &binding, source) {
                bindings.push(binding);
            }
        }
        bindings
    }

    fn accept(&mut self, field: &str, binding: &Binding, source: Source) -> bool {
        let reserved_prefix =
            binding.is_prefix() && self.main.prefixes.contains(&binding.combo.normalized());
        if reserved_prefix {
            if source == Source::Default && self.main.prefix_source == Source::User {
                return false;
            }
            self.note(format!(
                "reserved keybinding: {field} = {:?} uses keys.prefix as the prefix-mode key; pressing the prefix twice sends a literal prefix key, so this binding is disabled",
                binding.label
            ));
            return false;
        }
        if let Some(owner) = self.main.owner(binding) {
            if source == Source::Default && owner.source == Source::User {
                return false;
            }
            let kept = owner.field.clone();
            self.note(format!("{}: kept {kept}, disabled {field}", binding.label));
            return false;
        }
        if binding.is_direct() && binding.combo.is_unmodified_printable() {
            self.note(format!(
                "unsafe direct keybinding: {field} = {:?} would intercept typing; use {:?} to require the prefix; disabling binding",
                binding.label,
                format!("prefix+{}", binding.label)
            ));
            return false;
        }
        self.main.register(binding, field, source);
        true
    }

    fn accept_navigate(&mut self, field: &str, binding: &Binding, source: Source) -> bool {
        if binding.is_prefix() {
            self.note(format!(
                "navigate keybinding must not include prefix: {field} = {:?}; disabling binding",
                binding.label
            ));
            return false;
        }
        if binding.combo.normalized().code == KeyCode::Esc {
            self.note(format!(
                "navigate keybinding cannot use esc: {field} = {:?}; disabling binding",
                binding.label
            ));
            return false;
        }
        if let Some(owner) = self.navigate.owner(binding) {
            if source == Source::Default && owner.source == Source::User {
                return false;
            }
            let kept = owner.field.clone();
            self.note(format!("{}: kept {kept}, disabled {field}", binding.label));
            return false;
        }
        self.navigate.register(binding, field, source);
        true
    }

    fn commands(&mut self) -> Vec<CustomCommand> {
        let mut commands = Vec::new();
        for (index, config) in self.keys.command.iter().enumerate() {
            if config.command.trim().is_empty() {
                self.note(format!(
                    "empty custom command: keys.command[{index}].command; disabling custom command"
                ));
                continue;
            }
            if config.kind == CommandKind::PluginAction {
                self.note(format!(
                    "unsupported custom command: keys.command[{index}].type = \"plugin_action\"; drover has no plugins; disabling custom command"
                ));
                continue;
            }
            let field = format!("keys.command[{index}].key");
            let bindings = self.action(&field, config.key.values(), Source::User);
            if bindings.is_empty() {
                continue;
            }
            let popup = config.kind == CommandKind::Popup;
            if !popup && (config.width.is_some() || config.height.is_some()) {
                self.note(format!(
                    "popup size on non-popup custom command: keys.command[{index}]; ignoring width and height"
                ));
            }
            let label = bindings
                .iter()
                .map(|binding| binding.label.clone())
                .collect::<Vec<_>>()
                .join(" / ");
            commands.push(CustomCommand {
                index,
                bindings,
                label,
                command: config.command.clone(),
                kind: config.kind,
                description: config.description.clone(),
                width: config.width.filter(|_| popup),
                height: config.height.filter(|_| popup),
            });
        }
        commands
    }
}

fn parse_prefixes(keys: &KeysConfig) -> (Vec<KeyCombo>, Option<String>, Vec<String>) {
    let supplied = keys.prefix.is_some() || keys.extra_prefixes.is_some();
    let values: Vec<String> = if supplied {
        keys.prefix
            .iter()
            .chain(&keys.extra_prefixes)
            .flat_map(|binding| binding.values().iter().cloned())
            .collect()
    } else {
        vec![keys::DEFAULT_PREFIX.to_string()]
    };
    let mut combos = Vec::new();
    let mut diagnostics = Vec::new();
    for raw in values.iter().map(|raw| raw.trim()) {
        match KeyCombo::parse(raw) {
            Some(combo) if !combos.contains(&combo) => combos.push(combo),
            Some(_) => {}
            None => diagnostics.push(format!(
                "invalid keybinding: keys.prefix = {raw:?}; ignoring prefix"
            )),
        }
    }
    if combos.is_empty() {
        let error = format!(
            "invalid keybinding: keys.prefix; using fallback {}",
            DEFAULT_PREFIX_COMBO.label()
        );
        return (vec![DEFAULT_PREFIX_COMBO], Some(error), diagnostics);
    }
    (combos, None, diagnostics)
}

impl Keymap {
    pub fn build(keys: &KeysConfig) -> KeymapBuild {
        let (prefixes, prefix_error, diagnostics) = parse_prefixes(keys);
        let prefix_source = if keys.prefix.is_some() || keys.extra_prefixes.is_some() {
            Source::User
        } else {
            Source::Default
        };
        let mut navigate_registry = Registry::new(&prefixes, prefix_source);
        navigate_registry.reserve_navigate_keys();
        let mut builder = Builder {
            keys,
            main: Registry::new(&prefixes, prefix_source),
            navigate: navigate_registry,
            diagnostics,
        };
        let legacy = keys.indexed();
        let mut actions = BTreeMap::new();
        let mut navigate = BTreeMap::new();
        let mut indexed: [Vec<_>; 3] = Default::default();
        let mut commands = Vec::new();

        for source in [Source::User, Source::Default] {
            for (field, default, action) in NAVIGATE_FIELDS {
                if builder.source(field) == source {
                    let values = builder.values(field, default);
                    let field = format!("keys.{field}");
                    navigate.insert(*action, builder.navigate(&field, &values, source));
                }
            }
            for (field, default, slot) in ACTION_FIELDS {
                if builder.source(field) != source {
                    continue;
                }
                let values = builder.values(field, default);
                let name = format!("keys.{field}");
                match slot {
                    Slot::Action(action) => {
                        actions.insert(*action, builder.action(&name, &values, source));
                    }
                    Slot::Indexed(target) => {
                        let legacy_value = legacy_value(&legacy, *target);
                        if source == Source::Default && !legacy_value.trim().is_empty() {
                            continue;
                        }
                        indexed[target.slot()] = builder.indexed(&name, &values, source);
                    }
                }
            }
            let legacy_source = if keys.indexed.is_some() {
                Source::User
            } else {
                Source::Default
            };
            if legacy_source == source {
                for (target, field) in [
                    (Indexed::Tab, "keys.indexed.tabs"),
                    (Indexed::Workspace, "keys.indexed.workspaces"),
                    (Indexed::Agent, "keys.indexed.agents"),
                ] {
                    let extra =
                        builder.legacy_indexed(field, legacy_value(&legacy, target), source);
                    indexed[target.slot()].extend(extra);
                }
            }
            if source == Source::User {
                commands = builder.commands();
            }
        }

        let image_paste = image_paste(keys, &mut builder);

        KeymapBuild {
            keymap: Keymap {
                prefixes,
                actions,
                indexed,
                navigate,
                commands,
                image_paste,
            },
            diagnostics: builder.diagnostics,
            prefix_error,
        }
    }
}

fn legacy_value(legacy: &keys::IndexedKeys, target: Indexed) -> &str {
    match target {
        Indexed::Tab => &legacy.tabs,
        Indexed::Workspace => &legacy.workspaces,
        Indexed::Agent => &legacy.agents,
    }
}

fn image_paste(keys: &KeysConfig, builder: &mut Builder) -> Option<KeyCombo> {
    let raw = keys.remote_image_paste().trim();
    if raw.is_empty() {
        return None;
    }
    let combo = KeyCombo::parse(raw);
    if combo.is_none() {
        builder.note(format!(
            "invalid keybinding: keys.remote_image_paste = {raw:?}; disabling binding"
        ));
    }
    combo
}
