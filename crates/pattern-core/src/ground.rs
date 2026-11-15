//! What is on the fabric underneath the motifs.
//!
//! A pattern has three layers and always has: the fabric itself, which is slot
//! 0; a **ground** laid over it; and whatever is strewn on top. Bands were the
//! first ground, and the shape they needed turned out to be general: a
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
use crate::ornament::{Ornament, OrnamentField};
use crate::texture::{SlotField, Texture};

/// A generated layer between the fabric and the motifs.
///
/// Tagged in JSON by `kind`, so a document says what it is rather than being
/// identified by which fields happen to be present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Ground {
    /// Stripes, checks, ginghams and tartans.
    Bands(Bands),
    /// Reaction–diffusion: spots, worms, mazes and coral.
    Texture(Texture),
    /// Drawn by rule on a grid: Truchet curves, strapwork, square fractals.
    Ornament(Ornament),
}

impl Ground {
    /// Put this ground on a tile of a given size, ready to be sampled.
    ///
    /// The step exists because no ground can be sampled without knowing the
    /// tile it lives in. Bands have to be fitted to it or every join carries
    /// the remainder of the sequence; a texture has to be *grown* on it, which
    /// is a simulation rather than an adjustment. Both obligations are the same
    /// obligation, in that whatever a ground lays down has to close over the tile,
    /// and making it one call means neither can be skipped by accident.
    ///
    /// Growing a texture is the expensive thing this crate does. Call it once
    /// per render, not once per pixel.
    #[must_use]
    pub fn laid_over(&self, tile_width_mm: f32, tile_height_mm: f32) -> LaidGround {
        match self {
            Ground::Bands(b) => LaidGround::Bands(b.fitted_to(tile_width_mm, tile_height_mm).0),
            Ground::Texture(t) => LaidGround::Texture {
                field: t.grown(tile_width_mm, tile_height_mm),
                tile_width_mm,
                tile_height_mm,
            },
            Ground::Ornament(o) => {
                LaidGround::Ornament(OrnamentField::new(o, tile_width_mm, tile_height_mm))
            }
        }
    }

    /// Whether this ground puts anything on the fabric at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Ground::Bands(b) => b.is_empty(),
            Ground::Texture(t) => t.is_empty(),
            Ground::Ornament(o) => o.is_empty(),
        }
    }

    /// Every slot this ground needs a palette to define.
    #[must_use]
    pub fn slots_used(&self) -> Vec<u8> {
        match self {
            Ground::Bands(b) => b.slots_used(),
            Ground::Texture(t) => t.slots_used(),
            Ground::Ornament(o) => o.slots_used(),
        }
    }

    /// Rewrite the slots this ground paints with.
    ///
    /// `moved` answers where a slot went, or `None` for one that did not move.
    ///
    /// It lives on the enum, and the match is exhaustive with no default arm,
    /// because the caller cannot be trusted to remember. A palette resize
    /// replaces the palette wholesale and carries the ground's colours to the
    /// tail of the new one; a kind that was not renumbered keeps indices
    /// pointing into the *motif* colours and comes back in somebody else's
    /// colour. That is how a grown ground shipped rendering green, having been
    /// asked for blue, the first time the colour count moved: the carrying
    /// was general and the renumbering was written for bands only.
    ///
    /// [`GROUND_SLOT`](crate::GROUND_SLOT) is absence rather than a colour, so
    /// it is never in the map and never moves.
    pub fn renumber_slots(&mut self, moved: impl Fn(u8) -> Option<u8>) {
        let renumber = |slot: &mut u8| {
            if let Some(new) = moved(*slot) {
                *slot = new;
            }
        };
        match self {
            Ground::Bands(b) => {
                for band in b.vertical.iter_mut().chain(b.horizontal.iter_mut()) {
                    renumber(&mut band.slot);
                }
                for crossing in &mut b.crossings {
                    renumber(&mut crossing.a);
                    renumber(&mut crossing.b);
                    renumber(&mut crossing.slot);
                }
            }
            Ground::Texture(t) => {
                for level in &mut t.levels {
                    renumber(level);
                }
            }
            Ground::Ornament(o) => {
                for level in &mut o.levels {
                    renumber(level);
                }
            }
        }
    }

    /// The same ground at a different physical size.
    ///
    /// Everything measured in millimetres is multiplied; everything counted is
    /// left alone. A texture's cell count is across the tile, so it scales by
    /// following the tile and would be wrong to touch; a band's width and an
    /// ornament's cell are lengths, and would stay put while the fabric around
    /// them grew.
    #[must_use]
    pub fn scaled_by(&self, factor: f32) -> Self {
        match self {
            Ground::Bands(b) => {
                let widths = |seq: &[crate::Band]| -> Vec<crate::Band> {
                    seq.iter()
                        .map(|band| crate::Band::new(band.width_mm * factor, band.slot))
                        .collect()
                };
                Ground::Bands(Bands {
                    vertical: widths(&b.vertical),
                    horizontal: widths(&b.horizontal),
                    crossings: b.crossings.clone(),
                })
            }
            // Counted, not measured: `cells` is how many across the tile.
            Ground::Texture(t) => Ground::Texture(t.clone()),
            Ground::Ornament(o) => Ground::Ornament(Ornament {
                cell_mm: o.cell_mm * factor,
                stroke_mm: o.stroke_mm * factor,
                ..o.clone()
            }),
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
            Ground::Texture(_) | Ground::Ornament(_) => None,
        }
    }

    /// The texture, when that is what this is.
    #[must_use]
    pub fn as_texture(&self) -> Option<&Texture> {
        match self {
            Ground::Texture(t) => Some(t),
            Ground::Bands(_) | Ground::Ornament(_) => None,
        }
    }
}

/// A ground already laid on a tile, and therefore samplable.
///
/// Not serialised and not stored. A document keeps the parameters; this is what
/// they mean once a tile size is known, and it is thrown away with the render.
#[derive(Debug, Clone, PartialEq)]
pub enum LaidGround {
    Bands(Bands),
    Texture {
        field: SlotField,
        tile_width_mm: f32,
        tile_height_mm: f32,
    },
    Ornament(OrnamentField),
}

impl LaidGround {
    /// The slot painted at a point in the tile, or `None` where the fabric
    /// shows through.
    #[must_use]
    pub fn slot_at(&self, x_mm: f32, y_mm: f32) -> Option<u8> {
        match self {
            LaidGround::Bands(b) => b.slot_at(x_mm, y_mm),
            LaidGround::Texture {
                field,
                tile_width_mm,
                tile_height_mm,
            } => field.slot_at(x_mm, y_mm, *tile_width_mm, *tile_height_mm),
            LaidGround::Ornament(o) => o.slot_at(x_mm, y_mm),
        }
    }

    /// The share of the tile a slot actually covers, where that is knowable.
    ///
    /// `None` for bands, whose coverage is arithmetic on widths rather than a
    /// count of cells, and which nothing has needed.
    #[must_use]
    pub fn share_of(&self, slot: u8) -> Option<f32> {
        match self {
            LaidGround::Bands(_) | LaidGround::Ornament(_) => None,
            LaidGround::Texture { field, .. } => Some(field.share_of(slot)),
        }
    }
}

impl From<Bands> for Ground {
    fn from(bands: Bands) -> Self {
        Ground::Bands(bands)
    }
}

impl From<Texture> for Ground {
    fn from(texture: Texture) -> Self {
        Ground::Texture(texture)
    }
}

impl From<Ornament> for Ground {
    fn from(ornament: Ornament) -> Self {
        Ground::Ornament(ornament)
    }
}

impl Ground {
    /// The ornament, when that is what this is.
    #[must_use]
    pub fn as_ornament(&self) -> Option<&Ornament> {
        match self {
            Ground::Ornament(o) => Some(o),
            Ground::Bands(_) | Ground::Texture(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bands::Band;
    use crate::texture::Texture;

    #[test]
    fn a_ground_answers_for_whatever_kind_it_is() {
        let g = Ground::from(Bands::check(10.0, 10.0, 2, 3)).laid_over(240.0, 180.0);
        assert_eq!(g.slot_at(5.0, 5.0), Some(3), "both bands: the crossing");
        assert_eq!(g.slot_at(15.0, 15.0), None, "both gaps: the fabric");

        let g = Ground::from(Bands::check(10.0, 10.0, 2, 3));
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
        let LaidGround::Bands(bands) = g.laid_over(240.0, 180.0) else {
            unreachable!("bands stay bands");
        };
        let period = Bands::period(&bands.vertical);
        let repeats = 240.0 / period;
        assert!(
            (repeats - repeats.round()).abs() < 1e-4,
            "{repeats} repeats of a {period} mm sequence"
        );
    }

    #[test]
    fn renumbering_reaches_every_slot_of_every_kind() {
        // Slots are checked through `slots_used`, which is what a palette
        // resize consults to decide what to carry, so a slot the renumbering
        // missed is a slot the new palette has a colour for and the ground no
        // longer points at.
        let moved = |slot: u8| (slot != crate::GROUND_SLOT).then_some(slot + 100);

        let mut bands = Ground::from(Bands::check(10.0, 10.0, 2, 3));
        bands.renumber_slots(moved);
        assert_eq!(bands.slots_used(), vec![102, 103], "crossings move too");

        let mut texture = Ground::from(Texture {
            levels: vec![crate::GROUND_SLOT, 4, 5],
            ..Texture::default()
        });
        texture.renumber_slots(moved);
        assert_eq!(texture.slots_used(), vec![104, 105]);
        assert_eq!(
            texture.as_texture().expect("texture").levels[0],
            crate::GROUND_SLOT,
            "the fabric is absence, not a colour that moved"
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
