use std::collections::HashMap;

use pattern_core::{Motif, Palette, PatternError};
use tiny_skia::{Pixmap, PremultipliedColorU8};

/// Motifs available to a render, looked up by the `motif_id` on a placement.
///
/// Holds slot-indexed motifs, never resolved pixels: colour enters at render
/// time, from the pattern's palette. That is what makes a colourway a palette
/// swap rather than a re-ingest, and it means a bank can serve every colourway
/// of a range at once.
///
/// Reading files is the caller's job, which is what keeps this crate free of
/// I/O.
#[derive(Debug, Default, Clone)]
pub struct MotifBank {
    motifs: HashMap<String, Motif>,
}

impl MotifBank {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, motif: Motif) {
        self.motifs.insert(motif.id.clone(), motif);
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Motif> {
        self.motifs.get(id)
    }

    /// Change the physical size a motif stands for.
    ///
    /// Scale is a design decision, not a property of the file: the same
    /// drawing at 40 mm and at 90 mm is two different prints. Changing it must
    /// not mean matting and quantising the artwork again, so it is set on the
    /// resolved motif rather than back at ingest.
    pub fn set_natural_size(&mut self, id: &str, natural_size_mm: glam::Vec2) {
        if let Some(motif) = self.motifs.get_mut(id) {
            motif.natural_size_mm = natural_size_mm;
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.motifs.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.motifs.len()
    }

    /// Every motif held, in a stable order.
    ///
    /// Sorted by id because the backing map is a hash map, and a caller
    /// counting colours or listing a bank must not get a different answer
    /// between runs.
    #[must_use]
    pub fn motifs(&self) -> Vec<&Motif> {
        let mut all: Vec<&Motif> = self.motifs.values().collect();
        all.sort_by(|a, b| a.id.cmp(&b.id));
        all
    }
}

/// Resolve a slot-indexed motif into drawable pixels.
///
/// The one place slots become colours. Everything upstream of here — ingest,
/// storage, placement — deals in indices, which is why recolouring a whole
/// range costs one palette and no re-rendering of anything else.
pub fn resolve(motif: &Motif, palette: &Palette) -> Result<Pixmap, PatternError> {
    let rgba = motif.indexed.resolve_rgba(palette)?;
    let mut pixmap = Pixmap::new(motif.indexed.width.max(1), motif.indexed.height.max(1)).ok_or(
        PatternError::MotifChannelMismatch {
            width: motif.indexed.width,
            height: motif.indexed.height,
            slots: motif.indexed.slots.len(),
            alpha: motif.indexed.alpha.len(),
        },
    )?;

    for (target, source) in pixmap.pixels_mut().iter_mut().zip(rgba.chunks_exact(4)) {
        let a = source[3] as u32;
        // tiny-skia stores premultiplied alpha; the indexed form is straight,
        // because matting and quantisation both reason about colour
        // independently of coverage.
        let premultiply = |c: u8| ((c as u32 * a + 127) / 255) as u8;
        *target = PremultipliedColorU8::from_rgba(
            premultiply(source[0]),
            premultiply(source[1]),
            premultiply(source[2]),
            a as u8,
        )
        .unwrap_or_else(|| {
            PremultipliedColorU8::from_rgba(0, 0, 0, 0).expect("transparent is always valid")
        });
    }
    Ok(pixmap)
}
