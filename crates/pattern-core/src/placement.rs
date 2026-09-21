use glam::Vec2;
use serde::{Deserialize, Serialize};

/// One motif placed in the tile.
///
/// `position_mm` is tile-local and names the motif's *centre*, not a corner:
/// rotation and scale then behave the way a designer expects, turning the motif
/// in place instead of swinging it around an edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub motif_id: String,
    pub position_mm: Vec2,
    /// Degrees, clockwise. Degrees rather than radians because this value is
    /// authored by hand in JSON and read by people.
    #[serde(default)]
    pub rotation_deg: f32,
    /// Multiplier on the motif's natural physical size.
    #[serde(default = "unit_scale")]
    pub scale: f32,
    /// Draw the motif reflected left to right.
    ///
    /// Set by [`reflect`](crate::reflect) when a tile is built as a mirror
    /// repeat. Kept as a flag rather than folded into `rotation_deg` because a
    /// reflected rotation is a sign flip under one convention and an addition
    /// of 180 degrees under another; a flag cannot be wrong in that way, and it
    /// leaves the rotation reading as the angle a designer set.
    #[serde(default)]
    pub flip_x: bool,
    /// Draw the motif reflected top to bottom.
    #[serde(default)]
    pub flip_y: bool,
}

fn unit_scale() -> f32 {
    1.0
}

impl Placement {
    #[must_use]
    pub fn new(motif_id: impl Into<String>, position_mm: Vec2) -> Self {
        Self {
            motif_id: motif_id.into(),
            position_mm,
            rotation_deg: 0.0,
            scale: 1.0,
            flip_x: false,
            flip_y: false,
        }
    }
}

/// Bring a point into the tile's own coordinate range, `[0, W) x [0, H)`.
///
/// Wrapping by the tile rectangle rather than by the lattice vectors is correct
/// here and only here: the tile *is* a rectangle in every repeat this engine
/// supports, including the oblique ones. It is the neighbour offsets that must
/// come from the lattice, not this.
#[must_use]
pub fn wrap_into_tile(point_mm: Vec2, tile_size_mm: Vec2) -> Vec2 {
    Vec2::new(
        point_mm.x.rem_euclid(tile_size_mm.x),
        point_mm.y.rem_euclid(tile_size_mm.y),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: Vec2 = Vec2::new(240.0, 180.0);

    #[test]
    fn a_new_placement_is_unrotated_unscaled_and_unreflected() {
        let p = Placement::new("cherry", Vec2::new(10.0, 20.0));
        assert_eq!(p.rotation_deg, 0.0);
        assert_eq!(p.scale, 1.0);
        assert!(!p.flip_x && !p.flip_y);
    }

    #[test]
    fn points_inside_the_tile_are_untouched() {
        let p = Vec2::new(10.0, 20.0);
        assert_eq!(wrap_into_tile(p, TILE), p);
    }

    #[test]
    fn negative_coordinates_wrap_to_the_far_edge() {
        // rem_euclid, not %: the latter keeps the sign and puts the point
        // outside the tile it was meant to be brought into.
        assert_eq!(
            wrap_into_tile(Vec2::new(-10.0, -20.0), TILE),
            Vec2::new(230.0, 160.0)
        );
    }

    #[test]
    fn a_point_on_the_far_edge_wraps_to_zero() {
        assert_eq!(wrap_into_tile(TILE, TILE), Vec2::ZERO);
    }

    #[test]
    fn scale_defaults_to_one_when_absent_from_json() {
        // Guards the serde default: a placement authored without `scale` must
        // not deserialise to a zero-sized motif.
        let p: Placement =
            serde_json::from_str(r#"{"motif_id":"cherry","position_mm":[10.0,20.0]}"#)
                .expect("minimal placement should deserialise");
        assert_eq!(p.scale, 1.0);
        assert!(!p.flip_x && !p.flip_y);
    }
}
