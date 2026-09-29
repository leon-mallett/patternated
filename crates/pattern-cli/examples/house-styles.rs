//! Render candidate house styles, so they can be judged by eye rather than read.
//!
//! Not shipped surface — a drafting tool for the written style guide. Run with:
//!   cargo run -p pattern-cli --example house-styles -- <out-dir>
//!
//! Two independent dials, which is the point of the example:
//!
//! - an **idiom** decides how any subject is drawn — outline or none, flat or
//!   washed, how many shapes a motif resolves to;
//! - a **theme** decides subject matter and colour.
//!
//! They compose. A reference set is needed per idiom and a palette per theme,
//! so keeping them apart costs `idioms + themes` rather than
//! `idioms * themes`.
//!
//! The motifs are drawn in code rather than generated, deliberately: no model
//! is involved, nothing reaches the network, and the output is a function of
//! this file alone. Edit the palettes below and re-run.

use glam::Vec2;
use pattern_core::{
    scatter, Color, Figure, Ground, Growth, Lattice, Mirror, MotifWeight, Ornament, Palette,
    Pattern, ScatterParams, Texture, VectorMotif, VectorShape, GROUND_SLOT,
};
use pattern_render::{render_field, MotifBank};
use tiny_skia::{LineJoin, PathBuilder, PathStroker, Rect, Stroke, Transform};

/// The tile every concept is drawn on. A pyjama-ish repeat.
const TILE: Vec2 = Vec2::new(180.0, 180.0);

/// Palette roles, fixed across every theme.
///
/// This is the part of a style guide the engine can actually enforce. A theme
/// supplies five colours in these roles; an idiom decides which of them it
/// paints with. Because the roles are fixed, a motif drawn for one theme
/// recolours into another with no work — which is the colourway machinery
/// already in the engine, used for style rather than for a colour range.
mod slot {
    /// 0 is the cloth itself and is never painted.
    pub const GROUND_FIGURE: u8 = 1;
    pub const FILL: u8 = 2;
    pub const ACCENT: u8 = 3;
    pub const LINE: u8 = 4;
}

/// How a subject is drawn, independent of what the subject is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Idiom {
    /// Subject reduced to flat shape and colour. No outline. The spec's most
    /// portable row: a dinosaur and an ice cream resolve to the same grammar.
    FlatGeometric,
    /// A heavy uniform outline carrying the style, flat fill inside it. Also
    /// highly portable, because the outline is what reads as the style and it
    /// is indifferent to what it encloses.
    BoldOutline,
    /// Flat fill with a second, offset wash over it. As close to watercolour as
    /// a slot-based engine honestly gets — see the note in the generated
    /// README about why true watercolour fights the palette model.
    ChalkWash,
}

impl Idiom {
    fn name(self) -> &'static str {
        match self {
            Idiom::FlatGeometric => "flat-geometric",
            Idiom::BoldOutline => "bold-outline",
            Idiom::ChalkWash => "chalk-wash",
        }
    }
}

/// Subject matter and colour. Every theme fills the same five roles.
struct Theme {
    name: &'static str,
    /// Cloth, ground figure, fill, accent, line — in that order.
    colours: [Color; 5],
    ground: Ground,
}

fn themes() -> Vec<Theme> {
    vec![
        Theme {
            name: "starry-night",
            colours: [
                Color::new(26, 32, 74),
                Color::new(45, 55, 110),
                Color::new(247, 231, 176),
                Color::new(158, 194, 232),
                // Light, not dark. A near-black line on near-black cloth is
                // not a subtle outline, it is an absent one: the bold-outline
                // idiom disappeared entirely on this theme until the role was
                // read as "contrasts with the cloth" rather than "is dark".
                Color::new(214, 228, 248),
            ],
            // Soft scattered dots rather than Dust, which at this scale reads
            // as digital noise — square speckle, closer to a QR code than to
            // a sky.
            ground: Texture {
                growth: Growth::Spots,
                cells: 120,
                steps: 700,
                seed: 7,
                levels: vec![GROUND_SLOT, slot::GROUND_FIGURE],
            }
            .into(),
        },
        Theme {
            name: "deep-sea",
            colours: [
                Color::new(16, 54, 68),
                Color::new(28, 84, 96),
                Color::new(244, 176, 122),
                Color::new(126, 206, 196),
                Color::new(10, 34, 44),
            ],
            ground: Texture {
                growth: Growth::Coral,
                cells: 96,
                steps: 1800,
                seed: 11,
                levels: vec![GROUND_SLOT, slot::GROUND_FIGURE],
            }
            .into(),
        },
        Theme {
            name: "woodland-dusk",
            colours: [
                Color::new(42, 54, 40),
                Color::new(62, 82, 58),
                Color::new(226, 154, 72),
                Color::new(196, 210, 170),
                Color::new(28, 36, 28),
            ],
            // `levels[0]`, not `[GROUND_SLOT, ..]` as a texture would take.
            // The field is spelled the same on both kinds of ground and means
            // different things: a texture's levels are concentration bands, so
            // the lowest is properly the cloth; an ornament's are figure
            // levels, and putting the cloth sentinel first tells the figure to
            // paint nothing. This theme rendered with no ground at all until
            // somebody looked at it.
            ground: Ornament {
                figure: Figure::Scale,
                cell_mm: 22.0,
                depth: 2,
                stroke_mm: 2.0,
                variation: 0.2,
                seed: 3,
                levels: vec![slot::GROUND_FIGURE],
            }
            .into(),
        },
        Theme {
            name: "sherbet-sky",
            colours: [
                Color::new(252, 238, 228),
                Color::new(246, 214, 204),
                Color::new(238, 138, 146),
                Color::new(170, 168, 218),
                Color::new(96, 70, 92),
            ],
            ground: Texture {
                growth: Growth::Spots,
                cells: 72,
                steps: 900,
                seed: 5,
                levels: vec![GROUND_SLOT, slot::GROUND_FIGURE],
            }
            .into(),
        },
    ]
}

/// A five-pointed star, in a 0-100 box.
fn star_path() -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    // Written out rather than computed from sin/cos: the engine's determinism
    // invariant distrusts transcendentals, and a literal is also easier to nudge.
    let pts: [(f32, f32); 10] = [
        (50.0, 4.0),
        (61.0, 36.0),
        (96.0, 36.0),
        (68.0, 57.0),
        (79.0, 90.0),
        (50.0, 70.0),
        (21.0, 90.0),
        (32.0, 57.0),
        (4.0, 36.0),
        (39.0, 36.0),
    ];
    pb.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    pb.finish().expect("star is a closed polygon")
}

/// A crescent moon: a disc with a second disc bitten out of it.
fn moon_path() -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    pb.push_circle(50.0, 50.0, 44.0);
    pb.push_circle(68.0, 40.0, 38.0);
    pb.finish().expect("two circles")
}

/// A cloud: four overlapping discs.
///
/// A rectangle was the obvious floor and is wrong under an outline idiom: the
/// stroke traces its square corners, so the cloud reads as sitting on a shelf.
/// Discs all the way down means every contour the outline follows is curved.
fn cloud_path() -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    pb.push_circle(32.0, 58.0, 22.0);
    pb.push_circle(56.0, 48.0, 28.0);
    pb.push_circle(78.0, 58.0, 20.0);
    pb.push_circle(55.0, 66.0, 24.0);
    pb.finish().expect("cloud")
}

/// A four-pointed twinkle, concave where the star is convex.
fn twinkle_path() -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    pb.move_to(50.0, 6.0);
    pb.quad_to(56.0, 44.0, 94.0, 50.0);
    pb.quad_to(56.0, 56.0, 50.0, 94.0);
    pb.quad_to(44.0, 56.0, 6.0, 50.0);
    pb.quad_to(44.0, 44.0, 50.0, 6.0);
    pb.close();
    pb.finish().expect("twinkle")
}

fn shape(path: tiny_skia::Path, slot: u8, opacity: u8, even_odd: bool) -> VectorShape {
    VectorShape {
        path,
        slot,
        even_odd,
        opacity,
    }
}

/// Draw one subject in one idiom.
///
/// The whole idiom axis lives here. Same geometry, three grammars.
fn drawn(path: &tiny_skia::Path, idiom: Idiom, accent: bool, even_odd: bool) -> VectorMotif {
    let fill = if accent { slot::ACCENT } else { slot::FILL };
    let shapes = match idiom {
        Idiom::FlatGeometric => vec![shape(path.clone(), fill, 255, even_odd)],
        Idiom::BoldOutline => {
            // Stroke the path and fill the resulting outline, which is what
            // the engine says it does with vector artwork at import.
            //
            // The cheap version of this — scale the path up and lay it
            // underneath — was tried first and is wrong for any shape with a
            // hole: growing a crescent grows the bite out of it too, so the
            // outline wanders away from the edge it is supposed to trace. A
            // real stroke follows every contour, hole included.
            let mut stroker = PathStroker::new();
            let props = Stroke {
                width: 7.0,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            let outline = stroker
                .stroke(path, &props, 1.0)
                .expect("a closed path strokes");
            vec![
                // Non-zero, always: a stroke outline is a union of contours,
                // and even-odd would cancel them where they meet.
                shape(outline, slot::LINE, 255, false),
                shape(path.clone(), fill, 255, even_odd),
            ]
        }
        Idiom::ChalkWash => {
            let wash = path
                .clone()
                .transform(
                    Transform::from_translate(-50.0, -50.0)
                        .post_scale(0.82, 0.82)
                        .post_translate(56.0, 44.0),
                )
                .expect("uniform scale is invertible");
            vec![
                shape(path.clone(), fill, 210, even_odd),
                shape(
                    wash,
                    if accent { slot::FILL } else { slot::ACCENT },
                    130,
                    even_odd,
                ),
            ]
        }
    };
    VectorMotif {
        view: Rect::from_xywh(0.0, 0.0, 100.0, 100.0).expect("unit box"),
        shapes,
    }
}

fn bank_for(idiom: Idiom) -> MotifBank {
    let mut bank = MotifBank::new();
    // The fill rule is a property of the subject. Even-odd is what bites the
    // hole out of the crescent; applied to the cloud, whose three discs are
    // meant to *union*, it cancels every overlap and the cloud comes out full
    // of lens-shaped holes. Found by looking at the render, not by a test.
    let subjects: [(&str, tiny_skia::Path, bool, bool, f32); 4] = [
        ("star", star_path(), false, false, 26.0),
        ("moon", moon_path(), true, true, 34.0),
        ("cloud", cloud_path(), true, false, 38.0),
        ("twinkle", twinkle_path(), false, false, 18.0),
    ];
    for (id, path, accent, even_odd, size) in subjects {
        bank.insert_vector(id, Vec2::splat(size), drawn(&path, idiom, accent, even_odd))
            .expect("code-drawn motifs rasterise");
    }
    bank
}

fn pattern_for(theme: &Theme, bank: &MotifBank, seed: u64) -> Pattern {
    // Block, not half-drop: a ground needs a block lattice, because an
    // oblique repeat steps the ground by part of a tile at every join and a
    // grown texture then meets an edge it was not grown against.
    let lattice = Lattice::block(TILE);
    let mix: Vec<MotifWeight> = [
        ("star", 1.0, 26.0),
        ("moon", 0.35, 34.0),
        ("cloud", 0.5, 38.0),
        ("twinkle", 1.2, 18.0),
    ]
    .into_iter()
    .map(|(id, weight, size)| MotifWeight {
        motif_id: id.into(),
        weight,
        natural_size_mm: Vec2::splat(size),
        radius_fraction: 0.82,
    })
    .collect();
    let params = ScatterParams {
        density: 0.17,
        rotation_jitter_deg: 20.0,
        scale_range: (0.8, 1.2),
        motif_mix: mix,
        prevent_overlap: true,
        mirror: Mirror::None,
        on_axis_share: 0.0,
        seed,
    };
    let _ = bank;
    Pattern {
        tile_size_mm: TILE,
        lattice,
        placements: scatter(&params, TILE, &lattice),
        ground: Some(theme.ground.clone()),
        palette: Palette::new(theme.colours.to_vec()),
        seed,
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    std::fs::create_dir_all(&out).expect("output directory");

    let idioms = [Idiom::FlatGeometric, Idiom::BoldOutline, Idiom::ChalkWash];
    let themes = themes();
    let seed = 20260929;

    for idiom in idioms {
        let bank = bank_for(idiom);
        for theme in &themes {
            // The full grid is 12 renders and most of it is redundant to look
            // at: one theme across every idiom shows the idiom dial, and one
            // idiom across every theme shows the theme dial. The rest are
            // rendered anyway because they are cheap and somebody will want
            // to see a particular pair.
            let pattern = pattern_for(theme, &bank, seed);
            let field = render_field(&pattern, &bank, 96.0, 2, 2).expect("candidate renders");
            let path = format!("{out}/{}--{}.png", theme.name, idiom.name());
            field.save_png(&path).expect("write png");
            println!(
                "{path}  ({} placements, {} colours)",
                pattern.placements.len(),
                pattern_core::resolved_color_count(
                    &pattern,
                    &bank.motifs().into_iter().cloned().collect::<Vec<_>>()
                )
            );
        }
    }
}
