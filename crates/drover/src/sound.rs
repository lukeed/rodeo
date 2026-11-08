use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub use config::Sound;

const LIMIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(25);
const RATE: u32 = 22_050;
const HISTORY: usize = 16;
const MUTES: [&str; 3] = ["DROVER_DISABLE_SOUND", "HERDR_DISABLE_SOUND", "NEXTEST"];

static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Wav,
    Mp3,
}

struct Player {
    program: &'static str,
    args: &'static [&'static str],
    mp3: bool,
    wav: bool,
}

#[cfg(target_os = "macos")]
const PLAYERS: &[Player] = &[Player {
    program: "afplay",
    args: &[],
    mp3: true,
    wav: true,
}];

#[cfg(not(target_os = "macos"))]
const PLAYERS: &[Player] = &[
    Player {
        program: "paplay",
        args: &[],
        mp3: true,
        wav: true,
    },
    Player {
        program: "pw-play",
        args: &[],
        mp3: true,
        wav: true,
    },
    Player {
        program: "aplay",
        args: &["-q"],
        mp3: false,
        wav: true,
    },
    Player {
        program: "ffplay",
        args: &["-nodisp", "-autoexit", "-loglevel", "quiet"],
        mp3: true,
        wav: true,
    },
    Player {
        program: "mpg123",
        args: &["-q"],
        mp3: true,
        wav: false,
    },
    Player {
        program: "mpv",
        args: &["--no-video", "--really-quiet"],
        mp3: true,
        wav: true,
    },
];

#[derive(Debug, Default)]
pub struct Sounds {
    pub muted: bool,
    pub played: Vec<(Sound, Option<PathBuf>)>,
}

impl Sounds {
    pub fn from_env(env: &hc::Env) -> Self {
        Self {
            muted: MUTES
                .iter()
                .any(|key| env.get(key).is_some() || std::env::var_os(key).is_some()),
            played: Vec::new(),
        }
    }

    pub fn play(&mut self, sound: Sound, custom: Option<PathBuf>) {
        if self.played.len() == HISTORY {
            self.played.remove(0);
        }
        self.played.push((sound, custom.clone()));
        if self.muted {
            return;
        }
        let spawned = std::thread::Builder::new()
            .name("drover-sound".into())
            .spawn(move || {
                if let Some(path) = custom {
                    match play_file(&path, Kind::Mp3) {
                        Ok(()) => return,
                        Err(error) => {
                            tracing::warn!(path = %path.display(), %error, "custom sound failed; using the built-in one");
                        }
                    }
                }
                if let Err(error) = play_builtin(sound) {
                    tracing::debug!(%error, ?sound, "no sound played");
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(%error, "cannot start the sound thread");
        }
    }
}

fn play_builtin(sound: Sound) -> Result<(), String> {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("drover-sound-{}-{id}.wav", std::process::id()));
    std::fs::write(&path, wav(sound)).map_err(|e| e.to_string())?;
    let result = play_file(&path, Kind::Wav);
    let _ = std::fs::remove_file(&path);
    result
}

fn play_file(path: &Path, kind: Kind) -> Result<(), String> {
    let mut last = "no audio player found".to_owned();
    for player in PLAYERS {
        let fits = match kind {
            Kind::Wav => player.wav,
            Kind::Mp3 => player.mp3,
        };
        if !fits {
            continue;
        }
        match run(player, path) {
            Ok(true) => return Ok(()),
            Ok(false) => last = format!("{} failed", player.program),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => last = format!("{}: {error}", player.program),
        }
    }
    Err(last)
}

fn run(player: &Player, path: &Path) -> std::io::Result<bool> {
    let mut child = Command::new(player.program)
        .args(player.args)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status.success());
        }
        if started.elapsed() >= LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(false);
        }
        std::thread::sleep(POLL);
    }
}

struct Note {
    hz: f32,
    start: f32,
    length: f32,
}

fn notes(sound: Sound) -> &'static [Note] {
    match sound {
        Sound::Done => &[
            Note {
                hz: 659.25,
                start: 0.0,
                length: 0.18,
            },
            Note {
                hz: 987.77,
                start: 0.11,
                length: 0.34,
            },
        ],
        Sound::Request => &[
            Note {
                hz: 880.0,
                start: 0.0,
                length: 0.12,
            },
            Note {
                hz: 880.0,
                start: 0.15,
                length: 0.12,
            },
            Note {
                hz: 1174.66,
                start: 0.30,
                length: 0.28,
            },
        ],
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "audio samples are small, bounded numbers"
)]
pub fn samples(sound: Sound) -> Vec<i16> {
    let notes = notes(sound);
    let end = notes
        .iter()
        .map(|n| n.start + n.length)
        .fold(0.0_f32, f32::max);
    let count = (end * RATE as f32).ceil() as usize;
    (0..count)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let value: f32 = notes
                .iter()
                .filter(|n| t >= n.start && t < n.start + n.length)
                .map(|n| {
                    let local = t - n.start;
                    let attack = (local / 0.008).min(1.0);
                    let release = ((n.start + n.length - t) / 0.03).min(1.0);
                    let decay = (-local * 9.0).exp();
                    let phase = std::f32::consts::TAU * n.hz * local;
                    let tone = phase.sin() + 0.25 * (2.0 * phase).sin();
                    tone * attack * release * decay
                })
                .sum();
            (value * 0.35 * f32::from(i16::MAX)).clamp(f32::from(i16::MIN), f32::from(i16::MAX))
                as i16
        })
        .collect()
}

pub fn wav(sound: Sound) -> Vec<u8> {
    let samples = samples(sound);
    let data = u32::try_from(samples.len() * 2).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_sounds_are_short_valid_wavs() {
        for sound in [Sound::Done, Sound::Request] {
            let bytes = wav(sound);
            assert_eq!(&bytes[..4], b"RIFF");
            assert_eq!(&bytes[8..16], b"WAVEfmt ");
            let data = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
            assert_eq!(data as usize, bytes.len() - 44);
            let seconds = f64::from(data) / 2.0 / f64::from(RATE);
            assert!(seconds > 0.3 && seconds < 1.0, "{seconds}");
            let loudest = samples(sound).iter().map(|s| s.unsigned_abs()).max();
            assert!(loudest.is_some_and(|l| l > 5000 && l < 32_000));
        }
        assert_ne!(wav(Sound::Done), wav(Sound::Request));
    }

    #[test]
    fn the_env_mutes_playback() {
        for key in ["DROVER_DISABLE_SOUND", "HERDR_DISABLE_SOUND"] {
            assert!(Sounds::from_env(&hc::Env::new().with(key, "1")).muted);
        }
        let mut sounds = Sounds::from_env(&hc::Env::new().with("DROVER_DISABLE_SOUND", ""));
        sounds.play(Sound::Done, None);
        assert_eq!(sounds.played, [(Sound::Done, None)]);
    }
}
