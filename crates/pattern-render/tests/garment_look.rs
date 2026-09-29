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
    let out = std::path::PathBuf::from(
        "/tmp/claude-501/-Users-leon-Projects-pattern-engine/6413fcf8-733c-4904-9aa3-5f20aa7e6ec3/scratchpad",
    );
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
