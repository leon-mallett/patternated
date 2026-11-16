//! Every garment silhouette, written out to be looked at.
//!
//! A silhouette is judged by eye and by nothing else. The tests beside this
//! can say a shape is closed, is the right size and is not the same as another
//! one; none of them can say it looks like a pair of dungarees.
//!
//! `cargo test -p pattern-render --release --test garment_look -- --ignored`

mod support;

use glam::Vec2;
use pattern_core::{Lattice, Pattern};
use pattern_render::{render_mockup, Garment, MotifBank};
use support::{bank, palette, scattered_pattern, TILE};

#[test]
#[ignore = "writes pictures to judge the shapes by; run with --ignored"]
fn write_every_garment() {
    // CARGO_TARGET_TMPDIR is a real directory Cargo makes per package for
    // integration tests, so this works on anyone's machine and leaves nothing
    // outside target/. The path is printed because a test that writes pictures
    // for a person to look at is useless if they cannot find them.
    let out = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    eprintln!("writing garment pictures to {}", out.display());
    let bank: MotifBank = bank();
    let pattern = Pattern {
        tile_size_mm: TILE,
        lattice: Lattice::half_drop(TILE),
        placements: scattered_pattern(Lattice::half_drop(TILE), 7, 0.25).placements,
        ground: None,
        palette: palette(),
        seed: 7,
    };

    for garment in Garment::standard() {
        let shown = render_mockup(&pattern, &bank, &garment, 420).expect("should render");
        let slug: String = garment
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        std::fs::write(
            out.join(format!("garment-{slug}.png")),
            shown.pixmap.encode_png().expect("png"),
        )
        .expect("write");
        let e: Vec2 = shown.extent_mm;
        eprintln!("{:32} {:6.0} x {:6.0} mm", garment.name, e.x, e.y);
    }
}
