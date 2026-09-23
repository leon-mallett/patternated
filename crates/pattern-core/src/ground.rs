//! What is on the fabric underneath the motifs.
//!
//! A pattern has three layers and always has: the fabric itself, which is slot
//! 0; a **ground** laid over it; and whatever is strewn on top. Bands were the
//! first ground, and the shape they needed turned out to be general — a
//! function from a point in the tile to a slot, or to nothing where the fabric
//! shows through.
//!
//! That is the whole of the interface, and it is why a tartan and a texture are
//! the same kind of thing to everything downstream: both tile, both resolve
//! through the palette, both recolour with a colourway, both count toward what
//! a printer charges for.
//!
//! One ground, not a stack of them. Nothing has asked for a texture underneath
//! a check, and a list would be untested surface until something does.

use serde::{Deserialize, Serialize};

use crate::bands::Bands;

/// A generated layer between the fabric and the motifs.
///
/// Tagged in JSON by `kind`, so a document says what it is rather than being
/// identified by which fields happen to be present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Ground {
    /// Stripes, checks, ginghams and tartans.
    Bands(Bands),
}

impl Ground {
    /// The slot painted at a point, or `None` where the fabric shows.
    ///
    /// Expects a ground already fitted to the tile — see [`Self::fitted_to`].
    #[must_use]
    pub fn slot_at(&self, x_mm: f32, y_mm: f32) -> Option<u8> {
        match self {
            Ground::Bands(b) => b.slot_at(x_mm, y_mm),
        }
    }

    /// The same ground, adjusted so that whatever it lays down closes over the
    /// tile.
    ///
    /// Every ground has some version of this obligation, because every ground
    /// lives in the tile and repeats with it. For bands it is the band widths;
    /// for anything sampled on a grid it will be the grid.
    #[must_use]
    pub fn fitted_to(&self, tile_width_mm: f32, tile_height_mm: f32) -> Self {
        match self {
            Ground::Bands(b) => Ground::Bands(b.fitted_to(tile_width_mm, tile_height_mm).0),
        }
    }

    /// Whether this ground puts anything on the fabric at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Ground::Bands(b) => b.is_empty(),
        }
    }

    /// Every slot this ground needs a palette to define.
    #[must_use]
    pub fn slots_used(&self) -> Vec<u8> {
        match self {
            Ground::Bands(b) => b.slots_used(),
        }
    }

    /// The bands, when that is what this is.
    ///
    /// Present because the interface edits band sequences specifically, and a
    /// caller that wants to should not have to match on a shape it already
    /// knows.
    #[must_use]
    pub fn as_bands(&self) -> Option<&Bands> {
        match self {
            Ground::Bands(b) => Some(b),
        }
    }
}

impl From<Bands> for Ground {
    fn from(bands: Bands) -> Self {
        Ground::Bands(bands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bands::Band;

    #[test]
    fn a_ground_answers_for_whatever_kind_it_is() {
        let g = Ground::from(Bands::check(10.0, 10.0, 2, 3));
        assert_eq!(g.slot_at(5.0, 5.0), Some(3), "both bands: the crossing");
        assert_eq!(g.slot_at(15.0, 15.0), None, "both gaps: the fabric");
        assert_eq!(g.slots_used(), vec![2, 3]);
        assert!(!g.is_empty());
    }

    #[test]
    fn an_unpainted_ground_is_empty_whatever_kind_it_is() {
        assert!(Ground::from(Bands::default()).is_empty());
        assert!(Ground::from(Bands::stripe(10.0, 10.0, 0)).is_empty());
    }

    #[test]
    fn fitting_reaches_through_to_the_kind_underneath() {
        // 17 mm of sequence does not divide a 240 mm tile; fitting is what
        // makes it, and a ground that forgot to pass the call through would
        // tile with a band of the wrong width at every join.
        let g = Ground::from(Bands::check(9.0, 8.0, 1, 2));
        let fitted = g.fitted_to(240.0, 180.0);
        let bands = fitted.as_bands().expect("still bands");
        let period = Bands::period(&bands.vertical);
        let repeats = 240.0 / period;
        assert!(
            (repeats - repeats.round()).abs() < 1e-4,
            "{repeats} repeats of a {period} mm sequence"
        );
    }

    #[test]
    fn a_ground_says_what_it_is_in_json() {
        // Tagged rather than inferred from which fields are present, so a
        // document that gains a second kind of ground does not become
        // ambiguous.
        let g = Ground::Bands(Bands {
            vertical: vec![Band::new(10.0, 1)],
            ..Bands::default()
        });
        let json = serde_json::to_string(&g).expect("serialise");
        assert!(json.contains(r#""kind":"bands""#), "{json}");
        assert_eq!(
            serde_json::from_str::<Ground>(&json).expect("round trip"),
            g
        );
    }
}
