//! The shipped v2 axolotl must load through the same manifest the TUI uses,
//! with its own frames for every state (no silent fallback to v1 art).

use std::path::PathBuf;

use nahuali_tui::mascot::atlas::Atlas;
use nahuali_view::model::MascotState;

#[test]
fn shipped_v2_atlas_loads_every_state_with_its_own_frames() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/mascot/v2");
    let atlas = Atlas::load_dir(&dir).expect("assets/mascot/v2/atlas.json loads");
    for state in MascotState::ALL {
        let frames = atlas.frames(state);
        assert!(frames.len() >= 4, "{} has {} frames", state.name(), frames.len());
    }
    let idle = &atlas.frames(MascotState::Idle)[0].image;
    for state in MascotState::ALL {
        let frame = &atlas.frames(state)[0].image;
        assert_eq!(frame.dimensions(), idle.dimensions(), "{} shares the canvas", state.name());
    }
}
