//! Assemble one real Dreamwoven brief, to find out what assembly costs.
//!
//! Run with:
//!   cargo run -p pattern-cli --example dreamwoven-brief -- <out-dir>
//!
//! The brief:
//!
//! > I like ice cream, dinosaurs, monster trucks and unicorns. It's for
//! > pyjamas, so it should be night time.
//!
//! Which yields twelve motifs: several variants per interest, a small occasion
//! set that comes from "pyjamas" rather than from the child, and a dark navy
//! ground.
//!
//! # These silhouettes are deliberately crude
//!
//! They stand in for generated artwork, and they are drawn in code so that no
//! model is involved and nothing reaches the network. Crudeness is not a
//! problem for the question being asked: assembly is about size, visual weight
//! and how many of each thing there are, none of which care how well the
//! dinosaur is drawn. A beautifully drawn pile assembled wrongly still reads
//! as a sticker sheet.
//!
//! # The three tiers
//!
//! The brief produces a hierarchy on its own, and it is the structure the
//! scatter needs:
//!
//! - **Heroes** — the child's named interests. Large, few, heavy.
//! - **Secondaries** — the variants that stop repetition reading as repetition.
//! - **Fillers** — the occasion set. Small, numerous, and what actually makes
//!   a scatter look designed rather than sparse.
//!
//! # Knobs
//!
//! Density and seed are overridable from the environment — `D=0.45 S=3` — because
//! what this example exists to ask is what *varies* with them, and the answer for
//! asset coverage turned out to be "nothing".

use glam::Vec2;
use pattern_core::{
    Color, Lattice, Mirror, MotifWeight, Palette, Pattern, ScatterParams, VectorMotif, VectorShape,
};
use pattern_render::{render_field, MotifBank};
use tiny_skia::{PathBuilder, Rect};

const TILE: Vec2 = Vec2::new(300.0, 300.0);

/// One shared palette, nine slots, every motif quantised onto it.
///
/// This is the mechanism the spec rates highest after the style adapter, and
/// the reason is visible the moment twelve separately-drawn things are put on
/// one cloth: much of what reads as style mismatch is really colour mismatch.
/// Twelve generated motifs keeping their own colours is a collage; twelve
/// sharing nine slots is a design.
mod ink {
    pub const CREAM: u8 = 1;
    pub const PINK: u8 = 2;
    pub const MINT: u8 = 3;
    pub const AMBER: u8 = 4;
    pub const LILAC: u8 = 5;
    pub const TEAL: u8 = 6;
    pub const RUST: u8 = 7;
    pub const LINE: u8 = 8;
}

fn palette() -> Palette {
    Palette::new(vec![
        Color::new(20, 28, 58),    // 0 cloth — the night
        Color::new(245, 233, 200), // 1 cream
        Color::new(238, 143, 168), // 2 pink
        Color::new(127, 209, 185), // 3 mint
        Color::new(240, 169, 76),  // 4 amber
        Color::new(167, 155, 224), // 5 lilac
        Color::new(46, 94, 102),   // 6 teal
        Color::new(198, 90, 70),   // 7 rust
        Color::new(12, 18, 38),    // 8 line
    ])
}

// ---------------------------------------------------------------- primitives

fn circle(cx: f32, cy: f32, r: f32) -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    pb.finish().expect("circle")
}

fn oval(cx: f32, cy: f32, rx: f32, ry: f32) -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    pb.push_oval(Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0).expect("oval box"));
    pb.finish().expect("oval")
}

fn boxy(x: f32, y: f32, w: f32, h: f32) -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    pb.push_rect(Rect::from_xywh(x, y, w, h).expect("rect"));
    pb.finish().expect("rect")
}

fn poly(points: &[(f32, f32)]) -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    pb.move_to(points[0].0, points[0].1);
    for p in &points[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    pb.finish().expect("polygon")
}

fn motif(shapes: Vec<(tiny_skia::Path, u8)>) -> VectorMotif {
    VectorMotif {
        view: Rect::from_xywh(0.0, 0.0, 100.0, 100.0).expect("unit box"),
        shapes: shapes
            .into_iter()
            .map(|(path, slot)| VectorShape {
                path,
                slot,
                even_odd: false,
                opacity: 255,
            })
            .collect(),
    }
}

// -------------------------------------------------------------------- heroes

/// Long neck, long tail, small head.
fn sauropod(body: u8, belly: u8) -> VectorMotif {
    motif(vec![
        (
            poly(&[(26.0, 64.0), (8.0, 44.0), (4.0, 50.0), (20.0, 72.0)]),
            body,
        ), // tail
        (
            poly(&[(60.0, 58.0), (74.0, 20.0), (84.0, 22.0), (70.0, 62.0)]),
            body,
        ), // neck
        (oval(46.0, 62.0, 26.0, 16.0), body),
        (oval(46.0, 68.0, 19.0, 9.0), belly),
        (boxy(30.0, 74.0, 8.0, 18.0), body),
        (boxy(54.0, 74.0, 8.0, 18.0), body),
        (circle(86.0, 16.0, 8.0), body),
        (circle(89.0, 14.0, 1.8), ink::LINE),
    ])
}

/// Plates along the back are the whole read.
fn stegosaur(body: u8, plates: u8) -> VectorMotif {
    motif(vec![
        (
            poly(&[(24.0, 62.0), (4.0, 52.0), (6.0, 60.0), (22.0, 70.0)]),
            body,
        ),
        (oval(50.0, 62.0, 28.0, 17.0), body),
        (circle(79.0, 52.0, 11.0), body),
        (poly(&[(34.0, 46.0), (40.0, 26.0), (46.0, 46.0)]), plates),
        (poly(&[(48.0, 44.0), (55.0, 20.0), (62.0, 44.0)]), plates),
        (poly(&[(63.0, 46.0), (69.0, 28.0), (74.0, 47.0)]), plates),
        (boxy(34.0, 76.0, 9.0, 16.0), body),
        (boxy(58.0, 76.0, 9.0, 16.0), body),
        (circle(83.0, 50.0, 1.8), ink::LINE),
    ])
}

/// Big head, little arms.
fn tyrannosaur(body: u8, belly: u8) -> VectorMotif {
    motif(vec![
        (
            poly(&[(30.0, 56.0), (4.0, 40.0), (2.0, 48.0), (26.0, 66.0)]),
            body,
        ),
        (oval(48.0, 58.0, 24.0, 18.0), body),
        (oval(48.0, 64.0, 16.0, 10.0), belly),
        (
            poly(&[(62.0, 40.0), (92.0, 30.0), (94.0, 46.0), (64.0, 52.0)]),
            body,
        ),
        (poly(&[(66.0, 54.0), (74.0, 50.0), (72.0, 60.0)]), body), // arm
        (
            poly(&[(36.0, 74.0), (46.0, 74.0), (48.0, 94.0), (38.0, 94.0)]),
            body,
        ),
        (
            poly(&[(56.0, 74.0), (66.0, 74.0), (66.0, 94.0), (56.0, 94.0)]),
            body,
        ),
        (circle(86.0, 37.0, 2.0), ink::LINE),
    ])
}

/// Body, cab, and wheels far too big for it.
fn monster_truck(body: u8, trim: u8) -> VectorMotif {
    motif(vec![
        (boxy(16.0, 42.0, 68.0, 22.0), body),
        (
            poly(&[(28.0, 42.0), (40.0, 24.0), (62.0, 24.0), (68.0, 42.0)]),
            body,
        ),
        (
            poly(&[(34.0, 40.0), (42.0, 29.0), (58.0, 29.0), (62.0, 40.0)]),
            trim,
        ),
        (circle(30.0, 70.0, 20.0), ink::LINE),
        (circle(30.0, 70.0, 10.0), trim),
        (circle(72.0, 70.0, 20.0), ink::LINE),
        (circle(72.0, 70.0, 10.0), trim),
    ])
}

/// Horn, mane, and a body simple enough not to argue with them.
fn unicorn(body: u8, mane: u8, horn: u8) -> VectorMotif {
    motif(vec![
        (
            poly(&[(26.0, 58.0), (6.0, 44.0), (4.0, 52.0), (24.0, 66.0)]),
            mane,
        ),
        (oval(46.0, 60.0, 25.0, 16.0), body),
        (
            poly(&[(60.0, 52.0), (74.0, 26.0), (84.0, 30.0), (72.0, 58.0)]),
            body,
        ),
        (circle(80.0, 26.0, 11.0), body),
        (poly(&[(82.0, 16.0), (88.0, 2.0), (90.0, 18.0)]), horn),
        (
            poly(&[(62.0, 30.0), (76.0, 22.0), (78.0, 40.0), (64.0, 46.0)]),
            mane,
        ),
        (boxy(32.0, 72.0, 8.0, 20.0), body),
        (boxy(54.0, 72.0, 8.0, 20.0), body),
        (circle(85.0, 25.0, 1.8), ink::LINE),
    ])
}

// --------------------------------------------------------------- secondaries

fn ice_cream(scoop: u8, second: u8) -> VectorMotif {
    motif(vec![
        (
            poly(&[(32.0, 52.0), (68.0, 52.0), (50.0, 96.0)]),
            ink::AMBER,
        ),
        (circle(42.0, 44.0, 17.0), scoop),
        (circle(60.0, 42.0, 15.0), second),
        (circle(50.0, 28.0, 14.0), scoop),
        (circle(50.0, 13.0, 4.0), ink::PINK),
    ])
}

fn owl(body: u8, face: u8) -> VectorMotif {
    motif(vec![
        (poly(&[(24.0, 34.0), (34.0, 14.0), (42.0, 32.0)]), body),
        (poly(&[(58.0, 32.0), (66.0, 14.0), (76.0, 34.0)]), body),
        (oval(50.0, 56.0, 30.0, 34.0), body),
        (oval(50.0, 62.0, 20.0, 24.0), face),
        (circle(40.0, 48.0, 10.0), ink::CREAM),
        (circle(60.0, 48.0, 10.0), ink::CREAM),
        (circle(40.0, 48.0, 4.5), ink::LINE),
        (circle(60.0, 48.0, 4.5), ink::LINE),
        (
            poly(&[(46.0, 58.0), (54.0, 58.0), (50.0, 68.0)]),
            ink::AMBER,
        ),
    ])
}

// ------------------------------------------------------------------- fillers

/// A crescent, as one closed path of two arcs.
///
/// Not two circles with `even_odd`. Even-odd fills what is covered an odd
/// number of times, which subtracts one disc from another only while the
/// biting disc lies wholly inside the other — and a crescent needs it to
/// overlap the edge, so the part hanging outside gets *filled* rather than
/// removed. That turned the moon into a ring, and since the bite also reached
/// past the view box, the stray arc was clipped flat on the right as well.
///
/// Two arcs bounding one region has no such ambiguity, and stays in the box.
fn moon() -> VectorMotif {
    let mut pb = PathBuilder::new();
    pb.move_to(74.0, 16.0);
    // Outer edge, bulging left to about x = 23.
    pb.cubic_to(6.0, 20.0, 6.0, 80.0, 74.0, 84.0);
    // Inner edge, returning with a shallower bulge to about x = 50.
    pb.cubic_to(42.0, 78.0, 42.0, 22.0, 74.0, 16.0);
    pb.close();
    motif(vec![(pb.finish().expect("crescent"), ink::CREAM)])
}

fn star() -> VectorMotif {
    motif(vec![(
        poly(&[
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
        ]),
        ink::CREAM,
    )])
}

fn sparkle() -> VectorMotif {
    let mut pb = PathBuilder::new();
    pb.move_to(50.0, 6.0);
    pb.quad_to(56.0, 44.0, 94.0, 50.0);
    pb.quad_to(56.0, 56.0, 50.0, 94.0);
    pb.quad_to(44.0, 56.0, 6.0, 50.0);
    pb.quad_to(44.0, 44.0, 50.0, 6.0);
    pb.close();
    motif(vec![(pb.finish().expect("sparkle"), ink::CREAM)])
}

// ---------------------------------------------------------------------- tiers

#[derive(Clone, Copy)]
enum Tier {
    Hero,
    Secondary,
    Filler,
}

impl Tier {
    /// How often one of these turns up, relative to the others.
    ///
    /// Heroes are what the child asked for and are therefore the thing a
    /// pattern must not be crowded with: six large animals per tile reads as a
    /// toy box tipped out. Fillers are what make the gaps between heroes look
    /// intentional.
    fn weight(self) -> f32 {
        match self {
            Tier::Hero => 0.40,
            Tier::Secondary => 0.85,
            Tier::Filler => 2.6,
        }
    }

    /// Must it appear at all?
    ///
    /// Everything generated for the order, which is to say everything the
    /// child actually asked for. A filler is occasion dressing and turns up on
    /// weight alone; a hero that fails to turn up is the wrong product.
    fn required(self) -> bool {
        match self {
            Tier::Hero | Tier::Secondary => true,
            Tier::Filler => false,
        }
    }
}

struct Asset {
    id: &'static str,
    tier: Tier,
    size_mm: f32,
    art: VectorMotif,
}

fn brief() -> Vec<Asset> {
    vec![
        Asset {
            id: "sauropod",
            tier: Tier::Hero,
            size_mm: 60.0,
            art: sauropod(ink::MINT, ink::CREAM),
        },
        Asset {
            id: "stegosaur",
            tier: Tier::Hero,
            size_mm: 56.0,
            art: stegosaur(ink::LILAC, ink::CREAM),
        },
        Asset {
            id: "tyrannosaur",
            tier: Tier::Hero,
            size_mm: 58.0,
            art: tyrannosaur(ink::TEAL, ink::MINT),
        },
        Asset {
            id: "truck-amber",
            tier: Tier::Hero,
            size_mm: 54.0,
            art: monster_truck(ink::AMBER, ink::CREAM),
        },
        Asset {
            id: "truck-rust",
            tier: Tier::Hero,
            size_mm: 54.0,
            art: monster_truck(ink::RUST, ink::CREAM),
        },
        Asset {
            id: "unicorn",
            tier: Tier::Hero,
            size_mm: 56.0,
            art: unicorn(ink::CREAM, ink::PINK, ink::AMBER),
        },
        Asset {
            id: "cone-pink",
            tier: Tier::Secondary,
            size_mm: 34.0,
            art: ice_cream(ink::PINK, ink::CREAM),
        },
        Asset {
            id: "cone-mint",
            tier: Tier::Secondary,
            size_mm: 34.0,
            art: ice_cream(ink::MINT, ink::PINK),
        },
        Asset {
            id: "owl",
            tier: Tier::Secondary,
            size_mm: 36.0,
            art: owl(ink::LILAC, ink::CREAM),
        },
        Asset {
            id: "moon",
            tier: Tier::Filler,
            size_mm: 26.0,
            art: moon(),
        },
        Asset {
            id: "star",
            tier: Tier::Filler,
            size_mm: 17.0,
            art: star(),
        },
        Asset {
            id: "sparkle",
            tier: Tier::Filler,
            size_mm: 11.0,
            art: sparkle(),
        },
    ]
}

/// Build the pattern for a given set of asset sizes.
///
/// Split out so the same brief can be solved twice and the two layouts
/// compared — which is the only way to answer what a size slider in a buyer's
/// hands would actually do to the picture.
fn solve(assets: &[Asset], sizes: &[f32], seed: u64, density: f32) -> (Pattern, Vec<String>) {
    let mix: Vec<MotifWeight> = assets
        .iter()
        .zip(sizes)
        .map(|(a, size)| MotifWeight {
            motif_id: a.id.into(),
            weight: a.tier.weight(),
            required: a.tier.required(),
            natural_size_mm: Vec2::splat(*size),
            radius_fraction: 0.74,
        })
        .collect();

    let lattice = Lattice::block(TILE);
    let params = ScatterParams {
        density,
        rotation_jitter_deg: 10.0,
        scale_range: (0.9, 1.1),
        motif_mix: mix,
        prevent_overlap: true,
        mirror: Mirror::None,
        on_axis_share: 0.0,
        seed,
    };
    let outcome = pattern_core::scatter_with_outcome(&params, TILE, &lattice);
    (
        Pattern {
            tile_size_mm: TILE,
            lattice,
            placements: outcome.placements,
            ground: None,
            palette: palette(),
            seed,
        },
        outcome.unplaced_required,
    )
}

/// How much of one layout survives into another, 0..1.
///
/// A placement counts as surviving when the same motif is within a millimetre
/// of where it was. Deliberately strict: the question is whether a person
/// watching the preview would see a nudge or a reshuffle.
fn survival(before: &[pattern_core::Placement], after: &[pattern_core::Placement]) -> f32 {
    if before.is_empty() {
        return 1.0;
    }
    let kept = before
        .iter()
        .filter(|b| {
            after
                .iter()
                .any(|a| a.motif_id == b.motif_id && (a.position_mm - b.position_mm).length() < 1.0)
        })
        .count();
    kept as f32 / before.len() as f32
}

/// Pairs of placements whose ink now intersects, measured on the torus.
///
/// The same relation the sampler enforces while it is placing, asked after the
/// fact — which is what a control that moves nothing but size needs to know.
fn overlapping_pairs(pattern: &Pattern, assets: &[Asset]) -> usize {
    let radius = |pl: &pattern_core::Placement| -> f32 {
        let a = assets
            .iter()
            .find(|a| a.id == pl.motif_id)
            .expect("placement names an asset of this brief");
        Vec2::splat(a.size_mm).length() / 2.0 * 0.74 * pl.scale
    };
    let p = &pattern.placements;
    let mut hits = 0;
    for i in 0..p.len() {
        for j in (i + 1)..p.len() {
            let touching = radius(&p[i]) + radius(&p[j]);
            if pattern_core::lattice_distance_sq(
                p[i].position_mm,
                p[j].position_mm,
                &pattern.lattice,
            ) < touching * touching
            {
                hits += 1;
            }
        }
    }
    hits
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    std::fs::create_dir_all(&out).expect("output directory");

    let assets = brief();
    let mut bank = MotifBank::new();
    for a in &assets {
        // Anything outside the view box is clipped without a word, which is
        // how the moon came to have a straight edge down one side. Cheap to
        // assert, invisible otherwise.
        for (i, s) in a.art.shapes.iter().enumerate() {
            let b = s.path.bounds();
            let v = a.art.view;
            assert!(
                b.left() >= v.left()
                    && b.top() >= v.top()
                    && b.right() <= v.right()
                    && b.bottom() <= v.bottom(),
                "{}: shape {i} runs outside the view box and will be clipped \
                 ({:.1},{:.1})-({:.1},{:.1}) against ({:.1},{:.1})-({:.1},{:.1})",
                a.id,
                b.left(),
                b.top(),
                b.right(),
                b.bottom(),
                v.left(),
                v.top(),
                v.right(),
                v.bottom(),
            );
        }
        bank.insert_vector(a.id, Vec2::splat(a.size_mm), a.art.clone())
            .expect("code-drawn art rasterises");
    }

    let seed: u64 = std::env::var("S")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20261001);
    let density: f32 = std::env::var("D")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.30);

    let base: Vec<f32> = assets.iter().map(|a| a.size_mm).collect();
    let (pattern, unplaced) = solve(&assets, &base, seed, density);
    if !unplaced.is_empty() {
        println!(
            "COULD NOT PLACE: {} — the tile has no room for them",
            unplaced.join(", ")
        );
    }

    // `BIGGER=unicorn:1.4` re-solves with one asset resized, and reports how
    // much of the first layout survived. That is the question a size slider in
    // a buyer's hands actually asks.
    if let Ok(spec) = std::env::var("BIGGER") {
        let (id, factor) = spec.split_once(':').expect("BIGGER=<motif>:<factor>");
        let factor: f32 = factor.parse().expect("factor");
        let nudged: Vec<f32> = assets
            .iter()
            .zip(&base)
            .map(|(a, s)| if a.id == id { s * factor } else { *s })
            .collect();
        let (after, _) = solve(&assets, &nudged, seed, density);
        println!(
            "re-solved   {id} x{factor}: {} -> {} placements, {:.0}% of the layout survived",
            pattern.placements.len(),
            after.placements.len(),
            survival(&pattern.placements, &after.placements) * 100.0,
        );

        // The alternative: leave every position alone and change only the
        // placement's own scale. The layout survives entirely by construction,
        // so the only question is whether anything now collides.
        let mut in_place = pattern.clone();
        for pl in &mut in_place.placements {
            if pl.motif_id == id {
                pl.scale *= factor;
            }
        }
        println!(
            "scaled in place {id} x{factor}: 100% of the layout survived, {} overlapping pairs",
            overlapping_pairs(&in_place, &assets),
        );
    }

    let motifs: Vec<_> = bank.motifs().into_iter().cloned().collect();
    println!(
        "{} placements in a {}x{} mm tile, {} colours, alignment {:.2} (warns above {:.2})",
        pattern.placements.len(),
        TILE.x,
        TILE.y,
        pattern_core::resolved_color_count(&pattern, &motifs),
        pattern_core::alignment(&pattern.placements, TILE, &pattern.lattice),
        pattern_core::WORTH_A_LOOK,
    );

    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for p in &pattern.placements {
        *counts.entry(p.motif_id.as_str()).or_default() += 1;
    }
    for (id, n) in &counts {
        println!("  {id:>12}  {n}");
    }

    let field = render_field(&pattern, &bank, 96.0, 2, 2).expect("renders");
    let path = format!("{out}/brief.png");
    field.save_png(&path).expect("write png");
    println!("{path}");
}
