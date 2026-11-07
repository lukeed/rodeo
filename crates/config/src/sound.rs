use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Done,
    Request,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct SoundConfig {
    pub enabled: bool,
    pub path: Option<PathBuf>,
    pub done_path: Option<PathBuf>,
    pub request_path: Option<PathBuf>,
    pub agents: AgentSounds,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: None,
            done_path: None,
            request_path: None,
            agents: AgentSounds::default(),
        }
    }
}

impl SoundConfig {
    pub fn allows(&self, agent: Option<&str>) -> bool {
        self.enabled && self.agents.for_agent(agent) != AgentSound::Off
    }

    pub fn path_for(&self, sound: Sound, config_dir: &Path) -> Option<PathBuf> {
        let path = match sound {
            Sound::Done => self.done_path.as_ref(),
            Sound::Request => self.request_path.as_ref(),
        }
        .or(self.path.as_ref())?;
        let resolved = config_dir.join(path);
        file_problem(&resolved).is_none().then_some(resolved)
    }

    pub fn diagnostics(&self, config_dir: &Path) -> Vec<String> {
        [
            ("ui.sound.path", &self.path),
            ("ui.sound.done_path", &self.done_path),
            ("ui.sound.request_path", &self.request_path),
        ]
        .into_iter()
        .filter_map(|(field, path)| {
            let path = path.as_ref()?;
            let resolved = config_dir.join(path);
            let (kind, detail) = file_problem(&resolved)?;
            Some(format!(
                "{kind}: {field} = {} resolves to {}{detail}; using default sound",
                path.display(),
                resolved.display()
            ))
        })
        .collect()
    }
}

fn file_problem(path: &Path) -> Option<(&'static str, &'static str)> {
    let mp3 = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3"));
    if !mp3 {
        Some(("unsupported sound file format", "; expected an mp3 file"))
    } else if !path.exists() {
        Some(("missing sound file", ""))
    } else if !path.is_file() {
        Some(("invalid sound file", ""))
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSound {
    #[default]
    Default,
    On,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AgentSounds {
    pub pi: AgentSound,
    pub claude: AgentSound,
    pub codex: AgentSound,
    pub gemini: AgentSound,
    pub cursor: AgentSound,
    pub devin: AgentSound,
    pub agy: AgentSound,
    pub cline: AgentSound,
    pub open_code: AgentSound,
    pub github_copilot: AgentSound,
    pub kimi: AgentSound,
    pub kiro: AgentSound,
    pub droid: AgentSound,
    pub amp: AgentSound,
    pub grok: AgentSound,
    pub hermes: AgentSound,
    pub kilo: AgentSound,
    pub qodercli: AgentSound,
    pub qwen: AgentSound,
    pub letta: AgentSound,
    pub maki: AgentSound,
    pub muse: AgentSound,
}

impl Default for AgentSounds {
    fn default() -> Self {
        let on = AgentSound::Default;
        Self {
            pi: on,
            claude: on,
            codex: on,
            gemini: on,
            cursor: on,
            devin: on,
            agy: on,
            cline: on,
            open_code: on,
            github_copilot: on,
            kimi: on,
            kiro: on,
            droid: AgentSound::Off,
            amp: on,
            grok: on,
            hermes: on,
            kilo: on,
            qodercli: on,
            qwen: on,
            letta: on,
            maki: on,
            muse: on,
        }
    }
}

impl AgentSounds {
    pub fn for_agent(&self, agent: Option<&str>) -> AgentSound {
        match agent.unwrap_or_default() {
            "pi" => self.pi,
            "claude" => self.claude,
            "codex" => self.codex,
            "gemini" => self.gemini,
            "cursor" => self.cursor,
            "devin" => self.devin,
            "agy" => self.agy,
            "cline" => self.cline,
            "opencode" => self.open_code,
            "copilot" => self.github_copilot,
            "kimi" => self.kimi,
            "kiro" => self.kiro,
            "droid" => self.droid,
            "amp" => self.amp,
            "grok" => self.grok,
            "hermes" => self.hermes,
            "kilo" => self.kilo,
            "qodercli" => self.qodercli,
            "qwen" => self.qwen,
            "letta" => self.letta,
            "maki" => self.maki,
            "muse" => self.muse,
            _ => AgentSound::Default,
        }
    }
}
