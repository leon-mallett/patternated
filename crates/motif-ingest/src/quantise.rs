use pattern_core::{Color, IndexedMotif, Palette, PatternError};
use serde::{Deserialize, Serialize};

use crate::source::SourceImage;

/// How well a motif fitted the palette it was quantised onto.
///
/// The number that matters when forcing every motif onto one shared palette:
/// normalisation is what makes a colourway a single swap, and it is also what
/// silently ruins a motif whose colours are not represented. A pineapple's
/// leaves against a palette with no green will quantise to something, and
/// nothing about the resulting file says it went wrong. This does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantisationReport {
    /// Mean distance from each covered pixel to the colour it was given, 0..1.
    pub mean_error: f32,
    /// The worst single pixel, 0..1.
    pub max_error: f32,
    /// Palette slots the result actually uses.
    pub slots_used: Vec<u8>,
    /// Share of covered pixels that landed on the ground slot, 0..1.
    ///
    /// Tracked separately from the error because it is a different and worse
    /// failure. A motif can quantise onto the ground and score only a mediocre
    /// mean error while being, on the fabric, *invisible* — it is the
    /// background colour in the shape of a lemon.
    ///
    /// Observed rather than theorised: a yellow lemon ingested onto a palette
    /// of cream, red and green put 98% of its covered pixels on slot 0, because
    /// cream really is the nearest of those three to yellow. The metric was
    /// right and the result was useless.
    #[serde(default)]
    pub ground_share: f32,
}

impl QuantisationReport {
    /// Whether the fit is poor enough to be worth a person's attention.
    ///
    /// The threshold is a judgement, not a measurement, and it is deliberately
    /// generous: this exists to catch a motif that does not belong to the
    /// palette at all, not to grade near misses.
    #[must_use]
    pub fn is_poor_fit(&self) -> bool {
        self.mean_error > 0.12 || self.max_error > 0.45
    }

    /// Whether so much of the motif became the ground that it will not read.
    ///
    /// A third is already bad — that is a motif with a hole through it. Half
    /// means the palette has no colour for whatever this mostly is.
    #[must_use]
    pub fn dissolves_into_ground(&self) -> bool {
        self.ground_share > 0.33
    }
}

/// Perceptual-ish squared distance between two colours.
///
/// The "redmean" approximation: cheap, markedly better than plain RGB
/// Euclidean, and built only from multiply and add.
///
/// A proper perceptual space — Oklab, CIELAB — would quantise better. Both need
/// a cube root, and `f32` transcendentals carry no cross-platform bit-exactness
/// guarantee, which is the same constraint that shaped the scatter sampler.
/// Ingest runs once and stores its result, so the stakes are lower here than
/// there; the trade is revisited if real artwork quantises badly, and this
/// comment is the note to revisit it by.
#[must_use]
pub fn colour_distance_sq(a: Color, b: Color) -> f32 {
    let rbar = (a.r as f32 + b.r as f32) * 0.5;
    let dr = a.r as f32 - b.r as f32;
    let dg = a.g as f32 - b.g as f32;
    let db = a.b as f32 - b.b as f32;
    (2.0 + rbar / 256.0) * dr * dr + 4.0 * dg * dg + (2.0 + (255.0 - rbar) / 256.0) * db * db
}

/// Largest distance the metric can return, used to normalise errors into 0..1.
fn max_distance_sq() -> f32 {
    colour_distance_sq(Color::new(0, 0, 0), Color::new(255, 255, 255)).max(colour_distance_sq(
        Color::new(255, 0, 0),
        Color::new(0, 255, 255),
    ))
}

/// Map every pixel onto the nearest palette slot, keeping coverage intact.
///
/// Colour is quantised; alpha is not. A motif's edge stays as soft as it was
/// while its interior becomes strictly flat, which is both what a printed
/// illustration looks like and what keeps a four-colour motif from acquiring a
/// staircase outline.
pub fn quantise(
    image: &SourceImage,
    palette: &Palette,
) -> Result<(IndexedMotif, QuantisationReport), PatternError> {
    if palette.is_empty() {
        return Err(PatternError::EmptyPalette);
    }

    let mut slots = Vec::with_capacity(image.pixel_count());
    let mut alpha = Vec::with_capacity(image.pixel_count());
    let mut total_error = 0.0f32;
    let mut max_error = 0.0f32;
    let mut covered = 0usize;
    let mut on_ground = 0usize;
    let scale = max_distance_sq();

    for i in 0..image.pixel_count() {
        let p = i * 4;
        let source = Color::new(image.pixels[p], image.pixels[p + 1], image.pixels[p + 2]);
        let coverage = image.pixels[p + 3];

        let (slot, distance_sq) = nearest_slot(source, palette);
        slots.push(slot);
        alpha.push(coverage);

        // Error is only meaningful where something is actually drawn. A fully
        // transparent pixel keeps whatever colour the file had under it, and
        // scoring that would report the background rather than the motif.
        if coverage > 0 {
            let error = (distance_sq / scale).sqrt();
            total_error += error;
            max_error = max_error.max(error);
            covered += 1;
            if slot == crate::GROUND_SLOT {
                on_ground += 1;
            }
        }
    }

    let motif = IndexedMotif::new(image.width, image.height, slots, alpha)?;
    let report = QuantisationReport {
        mean_error: if covered > 0 {
            total_error / covered as f32
        } else {
            0.0
        },
        max_error,
        slots_used: motif.slots_used(),
        ground_share: if covered > 0 {
            on_ground as f32 / covered as f32
        } else {
            0.0
        },
    };
    Ok((motif, report))
}

/// The palette slot closest to a colour, and how far away it was.
fn nearest_slot(colour: Color, palette: &Palette) -> (u8, f32) {
    let mut best = (0u8, f32::MAX);
    for (index, candidate) in palette.colors.iter().enumerate() {
        let d = colour_distance_sq(colour, *candidate);
        if d < best.1 {
            best = (index as u8, d);
        }
    }
    best
}

/// Derive a palette of `count` colours from a set of images, by median cut.
///
/// Median cut rather than k-means because it is deterministic without needing a
/// seed, terminates in a fixed number of steps, and suits flat illustration —
/// where the colours are already clustered and the job is to find the clusters
/// rather than to discover structure.
///
/// Only covered pixels contribute. A scan's paper background would otherwise
/// dominate every bucket and hand back a palette of near-whites.
///
/// Slot 0 is the ground and is supplied rather than derived: the background of
/// a print is a design decision, not a property of the artwork.
#[must_use]
pub fn derive_palette(images: &[&SourceImage], ground: Color, count: usize) -> Palette {
    let mut colours: Vec<Color> = Vec::new();
    for image in images {
        for i in 0..image.pixel_count() {
            let p = i * 4;
            if image.pixels[p + 3] > 8 {
                colours.push(Color::new(
                    image.pixels[p],
                    image.pixels[p + 1],
                    image.pixels[p + 2],
                ));
            }
        }
    }

    let wanted = count.saturating_sub(1).max(1);
    let mut palette = vec![ground];
    if colours.is_empty() {
        return Palette::new(palette);
    }

    let mut boxes = vec![colours];
    while boxes.len() < wanted {
        // Split whichever box spans the most colour, so effort goes where the
        // variety is rather than being shared out evenly.
        let Some(index) = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .max_by(|a, b| {
                widest_span(a.1)
                    .1
                    .partial_cmp(&widest_span(b.1).1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
        else {
            break;
        };

        let mut bucket = boxes.swap_remove(index);
        let (channel, _) = widest_span(&bucket);
        bucket.sort_by_key(|c| channel_of(*c, channel));
        let mid = bucket.len() / 2;
        let upper = bucket.split_off(mid);
        boxes.push(bucket);
        boxes.push(upper);
    }

    // Stable order, so the same inputs always give the same slot numbering and
    // a motif ingested today indexes the same colours tomorrow.
    let mut derived: Vec<Color> = boxes
        .iter()
        .filter(|b| !b.is_empty())
        .map(|b| average(b))
        .collect();
    derived.sort_by_key(|c| (c.r, c.g, c.b));
    palette.extend(derived);
    Palette::new(palette)
}

fn channel_of(c: Color, channel: u8) -> u8 {
    match channel {
        0 => c.r,
        1 => c.g,
        _ => c.b,
    }
}

/// Which channel a set of colours spans most widely, and by how much.
fn widest_span(colours: &[Color]) -> (u8, f32) {
    let mut best = (0u8, -1.0f32);
    for channel in 0..3u8 {
        let mut lo = u8::MAX;
        let mut hi = 0u8;
        for c in colours {
            let v = channel_of(*c, channel);
            lo = lo.min(v);
            hi = hi.max(v);
        }
        let span = (hi as f32 - lo as f32) * if channel == 1 { 2.0 } else { 1.0 };
        if span > best.1 {
            best = (channel, span);
        }
    }
    best
}

fn average(colours: &[Color]) -> Color {
    let n = colours.len().max(1) as u32;
    let sum = colours.iter().fold((0u32, 0u32, 0u32), |acc, c| {
        (acc.0 + c.r as u32, acc.1 + c.g as u32, acc.2 + c.b as u32)
    });
    Color::new((sum.0 / n) as u8, (sum.1 / n) as u8, (sum.2 / n) as u8)
}
