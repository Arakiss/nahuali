//! Mascot frames loaded through a small manifest, so new art replaces old
//! art without code changes.
//!
//! A mascot directory holds `atlas.json` (schema in `assets/mascot/ATLAS.md`)
//! or, without a manifest, one folder per state with `frame-NN.png` files.
//! Search order: `$NAHUALI_MASCOT_DIR`, then the v2 art compiled into the
//! binary, then the v1 sheet compiled into the binary, which always works.
//! Nothing depends on the checkout the binary was built from.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::{GenericImageView, RgbaImage};
use nahuali_view::MascotState;
use rust_embed::RustEmbed;
use serde::Deserialize;

const EMBEDDED_ATLAS: &str = include_str!("../../../../assets/mascot/v1/atlas.json");
const EMBEDDED_SHEET: &[u8] = include_bytes!("../../../../assets/mascot/v1/spritesheet.png");
const DEFAULT_FRAME_MS: u32 = 140;
const V2_MANIFEST: &str = "atlas.json";
const ALPHA_VISIBLE: u8 = 16;

/// The v2 art: the manifest and the frames it names. Sheets, sources and
/// previews stay out of the binary.
#[derive(RustEmbed)]
#[folder = "../../assets/mascot/v2/"]
#[include = "atlas.json"]
#[include = "*/frame-*.png"]
struct EmbeddedV2;

/// One animation frame.
#[derive(Clone, Debug)]
pub struct Frame {
    pub image: Arc<RgbaImage>,
    /// How long it stays, in milliseconds.
    pub ms: u32,
    /// Extra random hold, `0..=jitter_ms`, so blinks never tick like a metronome.
    pub jitter_ms: u32,
}

/// Every state's frames, fallbacks resolved, all cropped to one shared box.
#[derive(Clone, Debug)]
pub struct Atlas {
    pub name: String,
    states: HashMap<MascotState, Vec<Frame>>,
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    name: String,
    #[serde(default)]
    sheets: HashMap<String, SheetSpec>,
    states: HashMap<String, StateSpec>,
    #[serde(default)]
    fallback: HashMap<String, String>,
}

#[derive(Deserialize)]
struct SheetSpec {
    file: String,
    grid: [u32; 2],
}

#[derive(Deserialize)]
struct StateSpec {
    frames: Vec<FrameSpec>,
}

#[derive(Deserialize)]
struct FrameSpec {
    file: Option<String>,
    sheet: Option<String>,
    cell: Option<u32>,
    rect: Option<[u32; 4]>,
    ms: Option<u32>,
    #[serde(default)]
    jitter_ms: u32,
}

/// Built-in fallbacks for states the art does not cover yet.
fn default_fallback(state: MascotState) -> MascotState {
    match state {
        MascotState::Alarmed => MascotState::Worried,
        MascotState::Greeting | MascotState::Proud | MascotState::Dreaming => MascotState::Happy,
        MascotState::Happy | MascotState::Worried => MascotState::Idle,
        _ => MascotState::Idle,
    }
}

impl Atlas {
    /// Find and load the best available art. Never fails: the embedded v1
    /// sheet is the last resort.
    pub fn discover() -> Self {
        let chosen = std::env::var_os("NAHUALI_MASCOT_DIR").map(PathBuf::from);
        chosen
            .and_then(|dir| Self::load_dir(&dir).ok())
            .or_else(|| Self::embedded_v2().ok())
            .unwrap_or_else(Self::embedded)
    }

    /// The v2 art compiled into the binary.
    pub fn embedded_v2() -> Result<Self, String> {
        let manifest = EmbeddedV2::get(V2_MANIFEST).ok_or("the v2 manifest is not embedded")?;
        let text = std::str::from_utf8(&manifest.data).map_err(|e| e.to_string())?;
        Self::from_manifest(text, |path| {
            EmbeddedV2::get(path).map(|file| file.data.into_owned()).ok_or(format!("{path} is not embedded"))
        })
    }

    /// Frames of v2 art in this binary, without decoding any of them.
    pub fn embedded_v2_frames() -> usize {
        EmbeddedV2::iter().filter(|path| path.ends_with(".png")).count()
    }

    /// The v1 sheet compiled into the binary (decoded once per process).
    pub fn embedded() -> Self {
        static EMBEDDED: std::sync::OnceLock<Atlas> = std::sync::OnceLock::new();
        EMBEDDED
            .get_or_init(|| {
                Self::from_manifest(EMBEDDED_ATLAS, |path| {
                    if path == "spritesheet.png" {
                        Ok(EMBEDDED_SHEET.to_vec())
                    } else {
                        Err(format!("{path} is not embedded"))
                    }
                })
                .expect("the embedded v1 atlas is valid")
            })
            .clone()
    }

    /// Load `dir/atlas.json`, or per-state folders of `frame-NN.png`.
    pub fn load_dir(dir: &Path) -> Result<Self, String> {
        let manifest = dir.join("atlas.json");
        let read = |path: &str| std::fs::read(dir.join(path)).map_err(|e| format!("{path}: {e}"));
        if manifest.is_file() {
            let text = std::fs::read_to_string(&manifest).map_err(|e| e.to_string())?;
            return Self::from_manifest(&text, read);
        }
        let mut states = HashMap::new();
        for state in MascotState::ALL {
            let folder = dir.join(state.name());
            let Ok(entries) = std::fs::read_dir(&folder) else {
                continue;
            };
            let mut files: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("frame-") && n.ends_with(".png"))
                })
                .collect();
            files.sort();
            let frames = files
                .iter()
                .map(|p| {
                    let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
                    Ok(Frame { image: Arc::new(decode(&bytes)?), ms: DEFAULT_FRAME_MS, jitter_ms: 0 })
                })
                .collect::<Result<Vec<_>, String>>()?;
            if !frames.is_empty() {
                states.insert(state, frames);
            }
        }
        Self::finish(dir.display().to_string(), states, &HashMap::new())
    }

    fn from_manifest(text: &str, read: impl Fn(&str) -> Result<Vec<u8>, String>) -> Result<Self, String> {
        let manifest: Manifest = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut sheets: HashMap<String, (RgbaImage, [u32; 2])> = HashMap::new();
        for (key, spec) in &manifest.sheets {
            sheets.insert(key.clone(), (decode(&read(&spec.file)?)?, spec.grid));
        }
        let mut files: HashMap<String, RgbaImage> = HashMap::new();
        let mut states = HashMap::new();
        for (name, spec) in &manifest.states {
            let state = MascotState::from_name(name).ok_or_else(|| format!("unknown state '{name}'"))?;
            let mut frames = Vec::new();
            for frame in &spec.frames {
                let image = if let Some(file) = &frame.file {
                    if !files.contains_key(file) {
                        files.insert(file.clone(), decode(&read(file)?)?);
                    }
                    files[file].clone()
                } else {
                    let key = frame.sheet.as_deref().ok_or("frame needs file or sheet")?;
                    let (sheet, grid) = sheets.get(key).ok_or(format!("unknown sheet {key}"))?;
                    let [x, y, w, h] = match (frame.rect, frame.cell) {
                        (Some(rect), _) => rect,
                        (None, Some(cell)) => {
                            let (cols, rows) = (grid[0].max(1), grid[1].max(1));
                            let (c, r) = (cell % cols, cell / cols);
                            if r >= rows {
                                return Err(format!("cell {cell} outside {cols}x{rows}"));
                            }
                            let left = c * sheet.width() / cols;
                            let top = r * sheet.height() / rows;
                            let right = (c + 1) * sheet.width() / cols;
                            let bottom = (r + 1) * sheet.height() / rows;
                            [left, top, right - left, bottom - top]
                        }
                        (None, None) => return Err("sheet frame needs cell or rect".into()),
                    };
                    if x + w > sheet.width() || y + h > sheet.height() {
                        return Err(format!("rect {x},{y},{w},{h} outside the sheet"));
                    }
                    sheet.view(x, y, w, h).to_image()
                };
                frames.push(Frame {
                    image: Arc::new(image),
                    ms: frame.ms.unwrap_or(DEFAULT_FRAME_MS).max(1),
                    jitter_ms: frame.jitter_ms,
                });
            }
            if !frames.is_empty() {
                states.insert(state, frames);
            }
        }
        let mut fallback = HashMap::new();
        for (from, to) in &manifest.fallback {
            let from = MascotState::from_name(from).ok_or(format!("unknown state '{from}'"))?;
            let to = MascotState::from_name(to).ok_or(format!("unknown state '{to}'"))?;
            fallback.insert(from, to);
        }
        Self::finish(manifest.name, states, &fallback)
    }

    fn finish(
        name: String,
        mut states: HashMap<MascotState, Vec<Frame>>,
        fallback: &HashMap<MascotState, MascotState>,
    ) -> Result<Self, String> {
        if states.is_empty() {
            return Err("no frames".into());
        }
        crop_to_union(&mut states);
        let base =
            states.get(&MascotState::Idle).or_else(|| states.values().next()).cloned().ok_or("no frames")?;
        for state in MascotState::ALL {
            if states.contains_key(&state) {
                continue;
            }
            let mut target = state;
            let mut frames = None;
            for _ in 0..MascotState::ALL.len() {
                target = fallback.get(&target).copied().unwrap_or_else(|| default_fallback(target));
                if let Some(found) = states.get(&target) {
                    frames = Some(found.clone());
                    break;
                }
            }
            states.insert(state, frames.unwrap_or_else(|| base.clone()));
        }
        Ok(Self { name, states })
    }

    pub fn frames(&self, state: MascotState) -> &[Frame] {
        self.states.get(&state).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Pixel aspect (width / height) shared by every frame.
    pub fn aspect(&self) -> f32 {
        let frame = &self.frames(MascotState::Idle)[0].image;
        frame.width() as f32 / frame.height().max(1) as f32
    }
}

fn decode(bytes: &[u8]) -> Result<RgbaImage, String> {
    image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map(|i| i.to_rgba8())
        .map_err(|e| e.to_string())
}

/// Crop each state's frames to the union of their visible pixels, then pad
/// every state to one shared size, centred horizontally and resting on the
/// bottom edge. Frames within a state never jump, and states that sit at
/// different heights on a sheet still stand on the same ground.
fn crop_to_union(states: &mut HashMap<MascotState, Vec<Frame>>) {
    let mut boxes: HashMap<MascotState, (u32, u32, u32, u32)> = HashMap::new();
    for (state, frames) in states.iter() {
        let mut bounds: Option<(u32, u32, u32, u32)> = None;
        for frame in frames {
            for (x, y, p) in frame.image.enumerate_pixels() {
                if p.0[3] >= ALPHA_VISIBLE {
                    bounds = Some(match bounds {
                        None => (x, y, x, y),
                        Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                    });
                }
            }
        }
        if let Some(b) = bounds {
            boxes.insert(*state, b);
        }
    }
    let width = boxes.values().map(|b| b.2 - b.0 + 1).max().unwrap_or(0);
    let height = boxes.values().map(|b| b.3 - b.1 + 1).max().unwrap_or(0);
    if width == 0 || height == 0 {
        return;
    }
    let pad = (width.max(height) / 60).max(1);
    let (canvas_w, canvas_h) = (width + 2 * pad, height + 2 * pad);
    for (state, frames) in states.iter_mut() {
        let Some(&(x0, y0, x1, y1)) = boxes.get(state) else {
            continue;
        };
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let left = i64::from((canvas_w - w) / 2);
        let top = i64::from(canvas_h - pad - h);
        for frame in frames.iter_mut() {
            // Sheets cut into cells may differ by a pixel; clamp per frame.
            let fw = w.min(frame.image.width().saturating_sub(x0));
            let fh = h.min(frame.image.height().saturating_sub(y0));
            let cropped = frame.image.view(x0, y0, fw, fh).to_image();
            let mut canvas = RgbaImage::new(canvas_w, canvas_h);
            image::imageops::overlay(&mut canvas, &cropped, left, top);
            frame.image = Arc::new(canvas);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_atlas_covers_every_state() {
        let atlas = Atlas::embedded();
        for state in MascotState::ALL {
            assert!(!atlas.frames(state).is_empty(), "{state:?}");
        }
        // The idle loop has a short blink frame and a jittered hold.
        let idle = atlas.frames(MascotState::Idle);
        assert!(idle.iter().any(|f| f.ms <= 150));
        assert!(idle.iter().any(|f| f.jitter_ms > 0));
        // Worried uses the second sheet row, so it differs from idle.
        assert_ne!(atlas.frames(MascotState::Worried)[0].image.as_raw(), idle[0].image.as_raw());
    }

    #[test]
    fn frames_are_cropped_to_the_character() {
        let atlas = Atlas::embedded();
        let image = &atlas.frames(MascotState::Idle)[0].image;
        // The raw cell is 429x458 with wide transparent margins.
        assert!(image.width() < 429 && image.height() < 458);
        let aspect = atlas.aspect();
        assert!((0.9..1.5).contains(&aspect), "aspect {aspect}");
    }

    #[test]
    fn per_state_folders_load_without_a_manifest() {
        let dir = std::env::temp_dir().join(format!("nahuali-atlas-{}", std::process::id()));
        let idle = dir.join("idle");
        std::fs::create_dir_all(&idle).unwrap();
        let mut image = RgbaImage::new(8, 8);
        image.put_pixel(3, 3, image::Rgba([255, 0, 0, 255]));
        image.save(idle.join("frame-01.png")).unwrap();
        image.put_pixel(4, 4, image::Rgba([0, 255, 0, 255]));
        image.save(idle.join("frame-02.png")).unwrap();
        let atlas = Atlas::load_dir(&dir).unwrap();
        assert_eq!(atlas.frames(MascotState::Idle).len(), 2);
        assert_eq!(atlas.frames(MascotState::Alarmed).len(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// `assets/mascot/v2/*/frame-*.png`.
    const EXPECTED_V2_FILES: usize = 43;

    #[test]
    fn the_v2_art_lives_in_the_binary() {
        let atlas = Atlas::embedded_v2().unwrap();
        assert!(atlas.name.contains("v2"), "{}", atlas.name);
        let on_disk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/mascot/v2");
        let disk = Atlas::load_dir(&on_disk).unwrap();
        for state in MascotState::ALL {
            let (embedded, read) = (atlas.frames(state), disk.frames(state));
            assert_eq!(embedded.len(), read.len(), "{state:?}");
            assert!(embedded.len() >= 4, "{state:?} has its own frames");
            assert_eq!(embedded[0].image.as_raw(), read[0].image.as_raw(), "{state:?}");
        }
        assert_eq!(Atlas::embedded_v2_frames(), EXPECTED_V2_FILES);
    }

    #[test]
    fn discovery_prefers_the_embedded_v2_art() {
        assert!(Atlas::discover().name.contains("v2"));
    }

    #[test]
    fn manifest_errors_are_reported() {
        let bad = r#"{"states":{"sleepy":{"frames":[{"file":"x.png"}]}}}"#;
        assert!(Atlas::from_manifest(bad, |_| Err("nope".into())).is_err());
    }
}
