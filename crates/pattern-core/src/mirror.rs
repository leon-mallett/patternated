//! Mirror repeats: the one classic repeat that is not a translation.
//!
//! Block, brick, half-drop and quarter-drop are all the same tile slid to a
//! different place, which is why [`Lattice`] describes them all. A mirror
//! repeat reflects instead, so no choice of `u` and `v` produces one and the
//! roadmap's name for this work, "mirror repeat lattice", was wrong about
//! where it lives.
//!
//! What a mirror repeat *is*, in terms this engine already has: the tile is
//! divided into two or four quadrants, one of them is designed, and the rest
//! are its reflections. The tile as a whole then repeats by ordinary block
//! translation. So the lattice stays [`Lattice::block`], the tile stays the
//! size the designer asked for, and the only new idea is that a placement can
//! carry a reflection.
//!
//! Two consequences worth stating, because both save work later:
//!
//! - **The tile size does not change.** The design cell is half the tile, or a
//!   quarter of it. A designer who asked for a 300 mm repeat still gets one;
//!   choosing a mirror subdivides that repeat rather than doubling it.
//! - **Seamlessness comes free twice over.** The reflected quadrants meet each
//!   other along the mirror axes by construction, and the tile meets its own
//!   translates because the cell is sampled on the torus. Nothing needs
//!   repairing at either join.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::lattice::Lattice;
use crate::placement::Placement;
use crate::torus::lattice_distance_sq;

/// Which axes a repeat reflects across.
///
/// `None` is a variant rather than an `Option<Mirror>` so that a document
/// written before mirrors existed deserialises to it without ceremony, and so
/// that the distance functions below can be called unconditionally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mirror {
    /// An ordinary repeat with no reflection.
    #[default]
    None,
    /// Reflected left to right. The design cell is the left half of the tile.
    ///
    /// The common one: it reads as formal without the heraldic stiffness of a
    /// four-way, and it is what "mirror repeat" usually means on a fabric.
    SideToSide,
    /// Reflected top to bottom. The design cell is the top half of the tile.
    TopToBottom,
    /// Reflected across both axes. The design cell is the top-left quarter.
    ///
    /// Strongly symmetrical, kaleidoscopic rather than strewn. Four copies of
    /// every motif means a quarter as many distinct ones at the same coverage,
    /// so it wants a smaller motif or a larger repeat than a scatter does.
    FourWay,
}

impl Mirror {
    /// Whether the tile is reflected left to right.
    #[must_use]
    pub fn reflects_x(self) -> bool {
        matches!(self, Mirror::SideToSide | Mirror::FourWay)
    }

    /// Whether the tile is reflected top to bottom.
    #[must_use]
    pub fn reflects_y(self) -> bool {
        matches!(self, Mirror::TopToBottom | Mirror::FourWay)
    }

    /// The part of the tile that is actually designed.
    ///
    /// Half the tile for a single mirror, a quarter for a four-way, the whole
    /// tile for no mirror at all.
    #[must_use]
    pub fn cell_size_mm(self, tile_size_mm: Vec2) -> Vec2 {
        Vec2::new(
            if self.reflects_x() {
                tile_size_mm.x / 2.0
            } else {
                tile_size_mm.x
            },
            if self.reflects_y() {
                tile_size_mm.y / 2.0
            } else {
                tile_size_mm.y
            },
        )
    }

    /// How many copies of each designed motif end up in the tile.
    #[must_use]
    pub fn copies(self) -> usize {
        match self {
            Mirror::None => 1,
            Mirror::SideToSide | Mirror::TopToBottom => 2,
            Mirror::FourWay => 4,
        }
    }

    /// The reflections that make up the tile, as `(flip_x, flip_y)` pairs.
    ///
    /// Always begins with the unreflected quadrant, so that the designed cell
    /// is the first copy of every motif and the order is stable.
    ///
    /// Public because a spatial index has to look for neighbours at every
    /// image of a point, not only at the point: under a mirror a motif can be
    /// reached through its reflection, and an index that only knows about the
    /// point itself would quietly stop finding those.
    #[must_use]
    pub fn quadrants(self) -> &'static [(bool, bool)] {
        match self {
            Mirror::None => &[(false, false)],
            Mirror::SideToSide => &[(false, false), (true, false)],
            Mirror::TopToBottom => &[(false, false), (false, true)],
            Mirror::FourWay => &[(false, false), (true, false), (false, true), (true, true)],
        }
    }
}

/// Expand placements designed in the cell into the whole tile.
///
/// The reflected copies carry the same motif at the same rotation, flagged so
/// the renderer reflects the drawn sprite. Reflection is recorded rather than
/// folded into the rotation deliberately: a reflected rotation is a sign flip
/// in one convention and an addition of 180 degrees in another, and the two
/// disagree in exactly the cases nobody checks. A pair of flags cannot be got
/// wrong that way, and it leaves `rotation_deg` meaning what a designer set.
#[must_use]
pub fn reflect(
    cell_placements: &[Placement],
    tile_size_mm: Vec2,
    mirror: Mirror,
) -> Vec<Placement> {
    if mirror == Mirror::None {
        return cell_placements.to_vec();
    }

    let mut out = Vec::with_capacity(cell_placements.len() * mirror.copies());
    for placement in cell_placements {
        for &(flip_x, flip_y) in mirror.quadrants() {
            let mut copy = placement.clone();
            if flip_x {
                copy.position_mm.x = tile_size_mm.x - placement.position_mm.x;
            }
            if flip_y {
                copy.position_mm.y = tile_size_mm.y - placement.position_mm.y;
            }
            copy.flip_x = flip_x;
            copy.flip_y = flip_y;
            out.push(copy);
        }
    }
    out
}

/// Squared distance between two points, counting reflections as well as
/// translations.
///
/// [`lattice_distance_sq`] answers how close two motifs get once every
/// *translated* copy is considered. Under a mirror there are reflected copies
/// too, and they are the ones that collide: two motifs either side of a mirror
/// axis are as close as their reflections put them, which is a pair that never
/// appears in the designed cell at all. Sampling a mirrored cell on the plain
/// lattice distance packs motifs right up to the axes and prints them
/// intersecting their own reflections.
///
/// Reduces to `lattice_distance_sq` exactly when there is no mirror, including
/// in its floating-point arithmetic, so a document without one is unaffected.
#[must_use]
pub fn mirror_distance_sq(a: Vec2, b: Vec2, lattice: &Lattice, mirror: Mirror) -> f32 {
    let mut best = lattice_distance_sq(a, b, lattice);
    for &(flip_x, flip_y) in mirror.quadrants() {
        if !flip_x && !flip_y {
            continue;
        }
        let image = Vec2::new(
            if flip_x { -b.x } else { b.x },
            if flip_y { -b.y } else { b.y },
        );
        best = best.min(lattice_distance_sq(a, image, lattice));
    }
    best
}

/// How close a point has to be to an axis to count as sitting *on* it.
///
/// Well under a pixel at any resolution this engine renders, since 0.085 mm is one
/// pixel at 300 DPI, so a position within this of an axis and one exactly on
/// it produce the same picture. Positions are snapped exactly by
/// [`snap_to_axis`]; the tolerance exists so that the comparison survives the
/// arithmetic rather than to admit near misses.
const COINCIDENT_MM: f32 = 1e-3;

/// Move a point onto the nearest mirror axis.
///
/// A motif sitting exactly on an axis is reflected onto itself. Half of it is
/// drawn and the reflection supplies the other half, so it comes out
/// bilaterally symmetrical: the damask move, and the only way to put anything
/// on a mirror line without the lopsided doubling that comes of straddling one
/// by an arbitrary amount.
///
/// Only one coordinate moves, the one already nearer to an axis. Snapping both
/// would put every such motif on a corner, where four reflections meet, and
/// that is a much stronger and rarer ornament than a designer asking for
/// motifs on the mirror lines has in mind.
///
/// The axes bounding the cell are at 0 and at the cell's own width or height,
/// which is half the tile. Both are real axes of the pattern and both need
/// ornaments: leaving one bare would put the corridor back on every second
/// mirror line.
#[must_use]
pub fn snap_to_axis(point_mm: Vec2, cell_size_mm: Vec2, mirror: Mirror) -> Vec2 {
    let nearer = |value: f32, extent: f32| if value * 2.0 <= extent { 0.0 } else { extent };

    let x = mirror
        .reflects_x()
        .then(|| nearer(point_mm.x, cell_size_mm.x));
    let y = mirror
        .reflects_y()
        .then(|| nearer(point_mm.y, cell_size_mm.y));

    match (x, y) {
        (Some(x), Some(y)) => {
            // Both axes available: take whichever the point is already closer
            // to, so the motif moves as little as possible and the sampler's
            // locality is not thrown away.
            if (point_mm.x - x).abs() <= (point_mm.y - y).abs() {
                Vec2::new(x, point_mm.y)
            } else {
                Vec2::new(point_mm.x, y)
            }
        }
        (Some(x), None) => Vec2::new(x, point_mm.y),
        (None, Some(y)) => Vec2::new(point_mm.x, y),
        (None, None) => point_mm,
    }
}

/// Squared distance from a point to its own nearest reflection.
///
/// A motif close to a mirror axis is close to *itself*, and nothing in a
/// pairwise separation test notices: the sampler never compares a candidate
/// with the copy it is about to spawn. Left out, motifs sit astride the axes
/// and come back doubled, which is the butterfly a mirror repeat is famous for when it
/// is not wanted.
///
/// A reflection that leaves the motif exactly where it is does not count. That
/// is a motif *on* an axis, which the reflection symmetrises rather than
/// collides with, and it is the whole of what [`snap_to_axis`] is for. A motif
/// that merely comes close to an axis is a different thing and is still
/// refused: it gets cut at the axis and doubled lopsidedly, which is the
/// butterfly nobody asked for.
///
/// Infinite when there is no mirror, so callers need no branch.
#[must_use]
pub fn self_image_distance_sq(point: Vec2, lattice: &Lattice, mirror: Mirror) -> f32 {
    let coincident_sq = COINCIDENT_MM * COINCIDENT_MM;
    let mut best = f32::INFINITY;
    for &(flip_x, flip_y) in mirror.quadrants() {
        if !flip_x && !flip_y {
            continue;
        }
        let image = Vec2::new(
            if flip_x { -point.x } else { point.x },
            if flip_y { -point.y } else { point.y },
        );
        let distance_sq = lattice_distance_sq(point, image, lattice);
        if distance_sq <= coincident_sq {
            continue;
        }
        best = best.min(distance_sq);
    }
    best
}

/// Fold a point back into the design cell by reflecting at its edges.
///
/// The cell is not a torus, because its edges are mirrors, so a candidate that walks
/// off the right-hand side belongs just inside it again, not over on the left.
/// Wrapping instead would still produce correct patterns, because every
/// candidate is distance-tested either way, but it scatters the sampler's
/// active front across the cell and slows the fill.
#[must_use]
pub fn fold_into_cell(point_mm: Vec2, cell_size_mm: Vec2, mirror: Mirror) -> Vec2 {
    Vec2::new(
        if mirror.reflects_x() {
            fold(point_mm.x, cell_size_mm.x)
        } else {
            point_mm.x.rem_euclid(cell_size_mm.x)
        },
        if mirror.reflects_y() {
            fold(point_mm.y, cell_size_mm.y)
        } else {
            point_mm.y.rem_euclid(cell_size_mm.y)
        },
    )
}

/// One axis of [`fold_into_cell`]: a triangle wave of period `2 * extent`.
fn fold(value: f32, extent: f32) -> f32 {
    if extent <= 0.0 {
        return 0.0;
    }
    let cycled = value.rem_euclid(extent * 2.0);
    if cycled <= extent {
        cycled
    } else {
        extent * 2.0 - cycled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: Vec2 = Vec2::new(240.0, 180.0);

    fn at(x: f32, y: f32) -> Placement {
        Placement::new("cherry", Vec2::new(x, y))
    }

    #[test]
    fn a_mirror_halves_the_designed_cell_without_changing_the_tile() {
        // The decision the whole module rests on. Choosing a mirror must not
        // silently double the repeat: a 240 mm repeat is a physical decision
        // about the print, not a consequence of how it was constructed.
        assert_eq!(Mirror::None.cell_size_mm(TILE), TILE);
        assert_eq!(
            Mirror::SideToSide.cell_size_mm(TILE),
            Vec2::new(120.0, 180.0)
        );
        assert_eq!(
            Mirror::TopToBottom.cell_size_mm(TILE),
            Vec2::new(240.0, 90.0)
        );
        assert_eq!(Mirror::FourWay.cell_size_mm(TILE), Vec2::new(120.0, 90.0));
    }

    #[test]
    fn reflecting_gives_one_copy_per_quadrant() {
        let cell = [at(30.0, 40.0), at(90.0, 20.0)];
        assert_eq!(reflect(&cell, TILE, Mirror::None).len(), 2);
        assert_eq!(reflect(&cell, TILE, Mirror::SideToSide).len(), 4);
        assert_eq!(reflect(&cell, TILE, Mirror::FourWay).len(), 8);
    }

    #[test]
    fn the_reflected_copy_lands_the_same_distance_the_other_side_of_the_axis() {
        let out = reflect(&[at(30.0, 40.0)], TILE, Mirror::FourWay);

        // 30 mm from the left edge becomes 30 mm from the right; the mirror
        // axis is the middle of the tile, at 120 mm.
        assert_eq!(out[0].position_mm, Vec2::new(30.0, 40.0));
        assert_eq!(out[1].position_mm, Vec2::new(210.0, 40.0));
        assert_eq!(out[2].position_mm, Vec2::new(30.0, 140.0));
        assert_eq!(out[3].position_mm, Vec2::new(210.0, 140.0));

        assert_eq!(
            out.iter().map(|p| (p.flip_x, p.flip_y)).collect::<Vec<_>>(),
            vec![(false, false), (true, false), (false, true), (true, true)]
        );
    }

    #[test]
    fn a_reflection_keeps_the_motif_its_rotation_and_its_scale() {
        // Rotation is deliberately *not* rewritten. Everything else about the
        // placement has to survive, or a mirrored scatter would lose the size
        // and rotation variation the sampler drew for it.
        let mut source = at(30.0, 40.0);
        source.rotation_deg = 17.0;
        source.scale = 1.3;

        for copy in reflect(&[source.clone()], TILE, Mirror::FourWay) {
            assert_eq!(copy.motif_id, "cherry");
            assert_eq!(copy.rotation_deg, 17.0);
            assert_eq!(copy.scale, 1.3);
        }
    }

    #[test]
    fn the_first_copy_is_always_the_one_that_was_designed() {
        for mirror in [
            Mirror::None,
            Mirror::SideToSide,
            Mirror::TopToBottom,
            Mirror::FourWay,
        ] {
            let out = reflect(&[at(30.0, 40.0)], TILE, mirror);
            assert_eq!(out[0].position_mm, Vec2::new(30.0, 40.0));
            assert!(!out[0].flip_x && !out[0].flip_y);
        }
    }

    #[test]
    fn without_a_mirror_the_distance_is_the_lattice_distance_bit_for_bit() {
        // Not "close to": identical. A mirror layout must not perturb every
        // document that does not use one, and the determinism tests compare
        // bytes.
        let l = Lattice::block(TILE);
        for (a, b) in [
            (Vec2::new(10.0, 10.0), Vec2::new(230.0, 95.0)),
            (Vec2::new(119.0, 3.0), Vec2::new(121.0, 177.0)),
            (Vec2::new(0.0, 0.0), Vec2::new(239.9, 179.9)),
        ] {
            assert_eq!(
                mirror_distance_sq(a, b, &l, Mirror::None),
                lattice_distance_sq(a, b, &l)
            );
        }
    }

    #[test]
    fn two_motifs_either_side_of_the_axis_are_as_close_as_their_reflections() {
        // The case the function exists for. Both points sit near the mirror
        // axis at x = 120; in the designed cell one of them does not exist at
        // all, so the plain lattice distance is measuring a pair that is never
        // printed.
        let l = Lattice::block(TILE);
        let a = Vec2::new(118.0, 50.0);
        let b = Vec2::new(115.0, 50.0);

        let plain = lattice_distance_sq(a, b, &l).sqrt();
        let mirrored = mirror_distance_sq(a, b, &l, Mirror::SideToSide).sqrt();

        assert!((plain - 3.0).abs() < 1e-3, "plain distance was {plain}");
        // b's reflection lands at 240 - 115 = 125, which is 7 mm from a, so
        // the reflection is not the nearer copy here...
        assert!((mirrored - 3.0).abs() < 1e-3);

        // ...but move a across the axis and it is.
        let a = Vec2::new(123.0, 50.0);
        let plain = lattice_distance_sq(a, b, &l).sqrt();
        let mirrored = mirror_distance_sq(a, b, &l, Mirror::SideToSide).sqrt();
        assert!((plain - 8.0).abs() < 1e-3, "plain distance was {plain}");
        assert!(
            (mirrored - 2.0).abs() < 1e-3,
            "reflection of b sits at 125 mm, 2 mm from a, but got {mirrored}"
        );
    }

    #[test]
    fn a_point_near_a_mirror_axis_is_near_itself() {
        let l = Lattice::block(TILE);

        // 4 mm from the axis at x = 120, so 8 mm from its own reflection.
        let d = self_image_distance_sq(Vec2::new(116.0, 50.0), &l, Mirror::SideToSide).sqrt();
        assert!((d - 8.0).abs() < 1e-3, "got {d}");

        // 3 mm from the tile edge, which is also a mirror axis: reflecting
        // about x = 0 and about x = 120 generate the same set of copies.
        let d = self_image_distance_sq(Vec2::new(3.0, 50.0), &l, Mirror::SideToSide).sqrt();
        assert!((d - 6.0).abs() < 1e-3, "got {d}");

        // Mid-cell, and so as far from itself as the cell allows.
        let d = self_image_distance_sq(Vec2::new(60.0, 50.0), &l, Mirror::SideToSide).sqrt();
        assert!((d - 120.0).abs() < 1e-3, "got {d}");
    }

    #[test]
    fn without_a_mirror_a_point_is_infinitely_far_from_its_reflections() {
        // There are none, and returning infinity rather than zero is what lets
        // the sampler apply the test unconditionally.
        let d = self_image_distance_sq(Vec2::new(116.0, 50.0), &Lattice::block(TILE), Mirror::None);
        assert!(d.is_infinite());
    }

    #[test]
    fn a_four_way_mirror_counts_the_diagonal_reflection_too() {
        let l = Lattice::block(TILE);
        // 5 mm from the vertical axis and 40 from the horizontal one. The
        // nearest image is the one reflected side to side, at 10 mm.
        let d = self_image_distance_sq(Vec2::new(115.0, 50.0), &l, Mirror::FourWay).sqrt();
        assert!((d - 10.0).abs() < 1e-3, "got {d}");

        // Equidistant from both axes: the diagonal image is the far one, at
        // sqrt(2) times the others, so the answer must not be it.
        let d = self_image_distance_sq(Vec2::new(115.0, 85.0), &l, Mirror::FourWay).sqrt();
        assert!((d - 10.0).abs() < 1e-3, "got {d}");
    }

    #[test]
    fn snapping_moves_a_point_onto_the_nearer_axis() {
        // A side-to-side mirror over a 240 mm tile has its axes at 0 and 120,
        // which are the two edges of the 120 mm cell. Both are real axes of
        // the pattern and both need ornaments.
        let cell = Mirror::SideToSide.cell_size_mm(TILE);
        let snap = |x: f32| snap_to_axis(Vec2::new(x, 40.0), cell, Mirror::SideToSide);

        assert_eq!(snap(20.0), Vec2::new(0.0, 40.0));
        assert_eq!(snap(100.0), Vec2::new(120.0, 40.0));
        // Dead centre goes to the nearer edge, which is the lower one.
        assert_eq!(snap(60.0), Vec2::new(0.0, 40.0));
        // The other coordinate is never touched by a single mirror.
        assert_eq!(snap(20.0).y, 40.0);
    }

    #[test]
    fn a_four_way_snap_moves_one_coordinate_not_both() {
        // Snapping both would put every mounted motif on a corner, where four
        // reflections meet, a much stronger ornament than "on the lines".
        let cell = Mirror::FourWay.cell_size_mm(TILE);
        assert_eq!(cell, Vec2::new(120.0, 90.0));

        // 8 mm from the vertical axis at 0, 40 from the horizontal one: the
        // vertical is nearer, so x moves and y stays.
        assert_eq!(
            snap_to_axis(Vec2::new(8.0, 40.0), cell, Mirror::FourWay),
            Vec2::new(0.0, 40.0)
        );
        // Turn it around and the other coordinate moves instead.
        assert_eq!(
            snap_to_axis(Vec2::new(50.0, 4.0), cell, Mirror::FourWay),
            Vec2::new(50.0, 0.0)
        );
    }

    #[test]
    fn snapping_without_a_mirror_changes_nothing() {
        let p = Vec2::new(37.0, 121.0);
        assert_eq!(snap_to_axis(p, TILE, Mirror::None), p);
    }

    #[test]
    fn a_motif_on_an_axis_is_allowed_to_meet_its_own_reflection() {
        // The rule the damask move rests on. A reflection that leaves the
        // motif exactly where it is symmetrises it; one that moves it must
        // clear it. Without the distinction, nothing could ever sit on a line.
        let l = Lattice::block(TILE);
        let cell = Mirror::SideToSide.cell_size_mm(TILE);

        for x in [0.0, 120.0] {
            let on_axis = snap_to_axis(Vec2::new(x, 50.0), cell, Mirror::SideToSide);
            assert!(
                self_image_distance_sq(on_axis, &l, Mirror::SideToSide).is_infinite(),
                "a motif at {on_axis:?} should be free of its own reflection"
            );
        }

        // A motif merely *near* a line is still refused: it gets cut at the
        // line and doubled lopsidedly, which is the butterfly nobody asked
        // for.
        let near = Vec2::new(4.0, 50.0);
        let d = self_image_distance_sq(near, &l, Mirror::SideToSide).sqrt();
        assert!((d - 8.0).abs() < 1e-3, "got {d}");
    }

    #[test]
    fn a_four_way_mount_still_has_to_clear_the_axis_it_is_not_on() {
        // Mounted on the vertical line, this motif coincides with its
        // side-to-side reflection, but its top-to-bottom reflection is a
        // genuinely different copy 20 mm away, and that one still counts.
        let l = Lattice::block(TILE);
        let on_vertical = Vec2::new(0.0, 80.0);
        let d = self_image_distance_sq(on_vertical, &l, Mirror::FourWay).sqrt();
        assert!((d - 20.0).abs() < 1e-3, "got {d}");
    }

    #[test]
    fn folding_reflects_at_the_cell_edge_rather_than_wrapping() {
        let cell = Vec2::new(120.0, 90.0);
        let m = Mirror::FourWay;

        // Inside: untouched.
        assert_eq!(
            fold_into_cell(Vec2::new(30.0, 40.0), cell, m),
            Vec2::new(30.0, 40.0)
        );
        // 5 mm past the right edge comes back to 5 mm inside it, not to 5 mm
        // from the left edge.
        assert_eq!(
            fold_into_cell(Vec2::new(125.0, 40.0), cell, m),
            Vec2::new(115.0, 40.0)
        );
        // And 5 mm before the left edge reflects to 5 mm inside.
        assert_eq!(
            fold_into_cell(Vec2::new(-5.0, 40.0), cell, m),
            Vec2::new(5.0, 40.0)
        );
        // Several cells out still lands inside.
        for x in [-500.0f32, -241.0, 0.0, 119.9, 361.0, 1000.0] {
            let folded = fold_into_cell(Vec2::new(x, 40.0), cell, m);
            assert!(
                (0.0..=cell.x).contains(&folded.x),
                "{x} folded to {} which is outside the cell",
                folded.x
            );
        }
    }

    #[test]
    fn an_unmirrored_axis_still_wraps() {
        // A side-to-side mirror reflects in x and repeats in y, so the two
        // axes are folded differently and the function has to do both.
        let cell = Vec2::new(120.0, 180.0);
        let folded = fold_into_cell(Vec2::new(125.0, 185.0), cell, Mirror::SideToSide);
        assert_eq!(folded, Vec2::new(115.0, 5.0));
    }
}
