//! Rasterising a [`Pattern`] to pixels at any resolution.
//!
//! Two ways to build a multi-tile field are offered, and the fact that they
//! agree pixel for pixel is what proves the repeat is seamless. See
//! [`render_field`] and [`render_field_direct`].

pub mod mockup;
pub mod motif;
mod pixel_lattice;
mod raster;

use std::sync::{Mutex, OnceLock};

use glam::{IVec2, Vec2};
use pattern_core::{wrap_into_tile, Color, Ground, LaidGround, Motif, Pattern, PatternError};
use thiserror::Error;
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, PremultipliedColorU8, Transform};

/// Re-exported because this crate's public API hands back its types: a caller
/// receiving a `Pixmap` has to be able to name one, and naming it through a
/// dependency of their own risks two versions of the same crate.
pub use tiny_skia;

pub use mockup::{render_mockup, Garment, Mockup};
pub use motif::{resolve, MotifBank};
pub use pixel_lattice::PixelLattice;
pub use raster::{draw_vector, RasterError};

/// Ways a render can fail.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error(transparent)]
    Pattern(#[from] PatternError),

    #[error("placement {index} refers to motif '{motif_id}', which is not in the bank")]
    UnknownMotif { index: usize, motif_id: String },

    #[error("a drawn motif could not be rasterised: {0}")]
    Vector(String),
    #[error("palette has no ground colour; slot 0 must be defined")]
    NoGroundColor,

    #[error("render target would be {width} x {height} px, which cannot be allocated")]
    Unallocatable { width: i32, height: i32 },

    #[error("dpi must be positive, got {0}")]
    NonPositiveDpi(f32),
}

/// Render a single tile: the artwork a print bureau actually receives.
///
/// Each placement is drawn at all nine lattice neighbour offsets and the result
/// clipped to the tile, so a motif crossing an edge arrives correctly on the
/// opposite one. There is no blending step and no seam to repair.
pub fn render_tile(pattern: &Pattern, bank: &MotifBank, dpi: f32) -> Result<Pixmap, RenderError> {
    pattern.validate()?;
    let pl = pixel_lattice_for(pattern, dpi)?;
    let mut target = new_pixmap(pl.tile_px)?;
    fill_ground(&mut target, pattern)?;
    fill_bands(&mut target, pattern, &pl)?;

    for stamp in sprites(pattern, bank, dpi, &pl)? {
        for offset in pl.neighbour_offsets() {
            stamp.draw(&mut target, offset);
        }
    }
    Ok(target)
}

/// Assemble a `cols x rows` field by laying rendered tiles at lattice offsets.
///
/// This is what the printer does with the file, so it is what a designer should
/// be shown. A single tile proves nothing about tracking; three by three is the
/// minimum at which the eye can find corridors.
pub fn render_field(
    pattern: &Pattern,
    bank: &MotifBank,
    dpi: f32,
    cols: u32,
    rows: u32,
) -> Result<Pixmap, RenderError> {
    let tile = render_tile(pattern, bank, dpi)?;
    let pl = pixel_lattice_for(pattern, dpi)?;
    let size = IVec2::new(pl.tile_px.x * cols as i32, pl.tile_px.y * rows as i32);
    let mut target = new_pixmap(size)?;

    for offset in pl.offsets_covering(size.x, size.y) {
        blit(&mut target, &tile, offset, IVec2::ZERO, size);
    }
    Ok(target)
}

/// The same field drawn straight from the pattern definition, with no tile in
/// between.
///
/// This is the reference: every placement drawn at every lattice offset
/// reaching the window, which is the definition of the repeat rather than an
/// implementation of it. [`render_field`] must match it exactly, and the test
/// asserting so is the one that would catch a tile whose edges do not join.
pub fn render_field_direct(
    pattern: &Pattern,
    bank: &MotifBank,
    dpi: f32,
    cols: u32,
    rows: u32,
) -> Result<Pixmap, RenderError> {
    pattern.validate()?;
    let pl = pixel_lattice_for(pattern, dpi)?;
    let size = IVec2::new(pl.tile_px.x * cols as i32, pl.tile_px.y * rows as i32);
    let mut target = new_pixmap(size)?;
    fill_ground(&mut target, pattern)?;
    fill_bands(&mut target, pattern, &pl)?;

    let offsets = pl.offsets_covering(size.x, size.y);
    for stamp in sprites(pattern, bank, dpi, &pl)? {
        for offset in &offsets {
            stamp.draw(&mut target, *offset);
        }
    }
    Ok(target)
}

/// The quantised lattice a render will use at this resolution.
pub fn pixel_lattice_for(pattern: &Pattern, dpi: f32) -> Result<PixelLattice, RenderError> {
    if dpi <= 0.0 {
        return Err(RenderError::NonPositiveDpi(dpi));
    }
    let reflects = reflected_axes(pattern);
    Ok(PixelLattice::for_mirror(
        &pattern.lattice,
        pattern.tile_size_mm,
        dpi,
        reflects.x,
        reflects.y,
    ))
}

/// Rasterise each placement once, and say where its top-left corner goes.
///
/// Every copy of a placement is then the same pixels at a whole-pixel offset.
/// That is what makes a tile and a directly drawn field agree exactly: were
/// each copy resampled at its own sub-pixel position, two copies of one motif
/// would differ slightly and the seam would show under inspection.
///
/// The cost is that positions quantise to whole pixels — 0.17 mm at 150 DPI,
/// which no fabric records.
///
/// The top-left corner rather than the centre, because a reflected placement
/// does not round the same way. A sprite of odd width centred on pixel `c`
/// sits half a pixel to its right; its mirror image belongs half a pixel to
/// the *left* of the reflected centre, and rounding both the same way puts the
/// two a whole pixel out of symmetry. Deciding the corner here keeps that
/// arithmetic in one place instead of in every caller.
fn sprites(
    pattern: &Pattern,
    bank: &MotifBank,
    dpi: f32,
    pl: &PixelLattice,
) -> Result<Vec<Stamp>, RenderError> {
    let k = pattern_core::px_per_mm(dpi);

    // Positions are quantised against the tile's own pixel size, not against
    // `k`. `PixelLattice` has already decided that the rounded integer tile
    // *is* the repeat and derived the lattice vectors from it; a position
    // scaled by `k` instead lives in a tile a fraction of a pixel wider or
    // narrower than the one it is drawn into. The difference is under a pixel
    // and shows up in exactly one place: reflect a position across a mirror
    // axis and the two roundings can land a whole pixel apart, which is a
    // visible break in the one repeat whose whole point is symmetry.
    let scale_mm_to_px = Vec2::new(
        pl.tile_px.x as f32 / pattern.tile_size_mm.x,
        pl.tile_px.y as f32 / pattern.tile_size_mm.y,
    );
    let reflects = reflected_axes(pattern);

    // Resolve each distinct motif once. A pattern is usually many placements of
    // a few motifs, so resolving per placement would redo the same slot lookup
    // for every copy on the tile.
    let mut resolved: std::collections::HashMap<&str, Vec<Pixmap>> =
        std::collections::HashMap::new();
    for placement in &pattern.placements {
        if resolved.contains_key(placement.motif_id.as_str()) {
            continue;
        }
        let motif = lookup(bank, pattern, &placement.motif_id)?;

        // A drawing is rasterised here, at the size this render will draw it,
        // rather than having been sampled once when it was imported. That is
        // the whole reason its outlines were kept: the same motif is sixty
        // pixels across in a preview and six hundred in a print, and it should
        // be exactly right in both rather than close in one of them.
        //
        // At the *largest* scale any copy of it uses, because the copies share
        // one chain and reducing is lossless where enlarging is not.
        let base = match bank.vector(&placement.motif_id) {
            Some(vector) => {
                let biggest = pattern
                    .placements
                    .iter()
                    .filter(|p| p.motif_id == placement.motif_id)
                    .fold(0.0f32, |most, p| most.max(p.scale));
                let px = |mm: f32| ((mm * biggest * k).round() as u32).max(1);
                let drawn = draw_vector(
                    vector,
                    px(motif.natural_size_mm.x),
                    px(motif.natural_size_mm.y),
                )
                .map_err(|e| RenderError::Vector(e.to_string()))?;
                motif::resolve(
                    &Motif {
                        id: motif.id.clone(),
                        natural_size_mm: motif.natural_size_mm,
                        indexed: drawn,
                    },
                    &pattern.palette,
                )?
            }
            None => motif::resolve(motif, &pattern.palette)?,
        };
        resolved.insert(placement.motif_id.as_str(), mip_chain(base));
    }

    let mut out = Vec::with_capacity(pattern.placements.len());
    for placement in &pattern.placements {
        let motif = lookup(bank, pattern, &placement.motif_id)?;
        let base = resolved
            .get(placement.motif_id.as_str())
            .expect("every motif was resolved above");
        let mut sprite = rasterise(base, motif, placement.scale, placement.rotation_deg, k)?;
        // Reflected after rasterising, not before, so that the four copies of
        // one mirrored motif are reversals of identical pixels rather than four
        // separate resamplings that agree only approximately.
        if placement.flip_x {
            sprite = flipped_x(&sprite);
        }
        if placement.flip_y {
            sprite = flipped_y(&sprite);
        }

        // Not wrapped on a reflected axis. A motif designed at the very edge
        // of the cell reflects to the far edge of the tile — coordinate
        // exactly `W` — and wrapping that to zero would file the copy in the
        // wrong half and clip it away entirely.
        let wrapped = wrap_into_tile(placement.position_mm, pattern.tile_size_mm);
        let center_mm = Vec2::new(
            if reflects.x {
                placement.position_mm.x
            } else {
                wrapped.x
            },
            if reflects.y {
                placement.position_mm.y
            } else {
                wrapped.y
            },
        );
        let center = IVec2::new(
            (center_mm.x * scale_mm_to_px.x).round() as i32,
            (center_mm.y * scale_mm_to_px.y).round() as i32,
        );
        let top_left = IVec2::new(
            anchor(center.x, sprite.width() as i32, placement.flip_x),
            anchor(center.y, sprite.height() as i32, placement.flip_y),
        );
        out.push(Stamp {
            sprite,
            top_left,
            band_min: IVec2::new(
                band_min(pl.tile_px.x, reflects.x, placement.flip_x),
                band_min(pl.tile_px.y, reflects.y, placement.flip_y),
            ),
            band_max: IVec2::new(
                band_max(pl.tile_px.x, reflects.x, placement.flip_x),
                band_max(pl.tile_px.y, reflects.y, placement.flip_y),
            ),
        });
    }
    Ok(out)
}

/// A rasterised placement, where it goes, and how much of the tile it may
/// cover.
struct Stamp {
    sprite: Pixmap,
    /// Where the sprite's top-left corner goes, in tile-local pixels.
    top_left: IVec2,
    /// The half of the tile this copy owns, in tile-local pixels. The whole
    /// tile when the axis is not reflected.
    band_min: IVec2,
    band_max: IVec2,
}

impl Stamp {
    /// Draw this copy into `target`, offset by one lattice translate.
    ///
    /// The band travels with the sprite. A copy owns the same half of every
    /// tile it appears in, so clipping it in tile-local pixels and shifting
    /// both by the offset is the whole of it — which is also why the two
    /// render routes stay in agreement.
    fn draw(&self, target: &mut Pixmap, offset: IVec2) {
        blit(
            target,
            &self.sprite,
            self.top_left + offset,
            self.band_min + offset,
            self.band_max + offset,
        );
    }
}

/// Which axes this pattern is reflected in, read from the placements rather
/// than stored.
///
/// The pattern *is* its placements: a tile with a reflected copy in it is a
/// mirror repeat, and there is nowhere else for the fact to live without
/// giving `Pattern` a mode flag that could disagree with them.
fn reflected_axes(pattern: &Pattern) -> glam::BVec2 {
    glam::BVec2::new(
        pattern.placements.iter().any(|p| p.flip_x),
        pattern.placements.iter().any(|p| p.flip_y),
    )
}

/// Room enough to stand for "no limit" while surviving being offset by a few
/// tiles. Not `i32::MAX`, which would overflow the moment a lattice offset is
/// added to it.
const UNBOUNDED: i32 = i32::MAX / 4;

/// The near edge of the half-tile a copy owns.
///
/// Unbounded on an axis that is not reflected, and that is not the same as
/// "the tile". A repeat is periodic, so a motif near one edge legitimately
/// draws past the opposite one — that overhang is how a tile's edges join at
/// all. Only a reflected axis bounds a copy, because there the far side
/// belongs to the reflection and is drawn by it.
fn band_min(extent: i32, reflected_axis: bool, flipped: bool) -> i32 {
    match (reflected_axis, flipped) {
        (true, true) => extent / 2,
        (true, false) => 0,
        (false, _) => -UNBOUNDED,
    }
}

/// The far edge of the half-tile a copy owns.
fn band_max(extent: i32, reflected_axis: bool, flipped: bool) -> i32 {
    match (reflected_axis, flipped) {
        (true, true) => extent,
        (true, false) => extent / 2,
        (false, _) => UNBOUNDED,
    }
}

/// Find a motif, naming the placement that wanted it when it is missing.
fn lookup<'a>(
    bank: &'a MotifBank,
    pattern: &Pattern,
    motif_id: &str,
) -> Result<&'a Motif, RenderError> {
    bank.get(motif_id).ok_or_else(|| {
        let index = pattern
            .placements
            .iter()
            .position(|p| p.motif_id == motif_id)
            .unwrap_or(0);
        RenderError::UnknownMotif {
            index,
            motif_id: motif_id.to_string(),
        }
    })
}

/// Successively halved copies of a motif, largest first.
///
/// Artwork arrives at illustration resolution — a thousand pixels or more —
/// and is drawn at a fraction of that: 78 mm at preview scale is under a
/// hundred pixels. Bilinear sampling reads a 2x2 neighbourhood, so reducing by
/// eight in one step looks at four source pixels out of every sixty-four and
/// throws the rest away. A dark keyline a few pixels wide falls between
/// samples and simply disappears in places, which is why a preview looked
/// coarser than the artwork rather than merely smaller.
///
/// Halving by box average loses nothing: every source pixel contributes to the
/// level below. Reducing to within a factor of two of the target and letting
/// bilinear do the rest is the standard answer, and it costs about a third of
/// the base again in memory.
///
/// Integer arithmetic throughout, so the chain is identical on any platform —
/// this sits upstream of everything a seed is supposed to reproduce.
fn mip_chain(base: Pixmap) -> Vec<Pixmap> {
    let mut levels = vec![base];
    while levels
        .last()
        .is_some_and(|l| l.width() > 1 || l.height() > 1)
    {
        let Some(next) = halved(levels.last().expect("just checked")) else {
            break;
        };
        levels.push(next);
    }
    levels
}

/// One level of the chain: a 2x2 box average, rounding half up.
///
/// Channels stay premultiplied, which is exactly the representation in which
/// averaging is correct — averaging unpremultiplied colour would let a fully
/// transparent pixel's arbitrary colour bleed into its neighbours.
///
/// An odd dimension rounds up rather than dropping the final row or column, so
/// the motif keeps its full extent; that last output pixel averages the one
/// source pixel available to it.
fn halved(src: &Pixmap) -> Option<Pixmap> {
    let (sw, sh) = (src.width(), src.height());
    let (w, h) = (sw.div_ceil(2).max(1), sh.div_ceil(2).max(1));
    if w == sw && h == sh {
        return None;
    }

    let mut out = Pixmap::new(w, h)?;
    let sp = src.pixels();
    let dp = out.pixels_mut();
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b, mut a, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for dy in 0..2 {
                for dx in 0..2 {
                    let (sx, sy) = (x * 2 + dx, y * 2 + dy);
                    if sx >= sw || sy >= sh {
                        continue;
                    }
                    let p = sp[(sy * sw + sx) as usize];
                    r += p.red() as u32;
                    g += p.green() as u32;
                    b += p.blue() as u32;
                    a += p.alpha() as u32;
                    n += 1;
                }
            }
            let mean = |v: u32| ((v + n / 2) / n.max(1)) as u8;
            dp[(y * w + x) as usize] =
                PremultipliedColorU8::from_rgba(mean(r), mean(g), mean(b), mean(a))
                    .unwrap_or(sp[((y * 2).min(sh - 1) * sw + (x * 2).min(sw - 1)) as usize]);
        }
    }
    Some(out)
}

/// The smallest level that is still at least as large as the target.
///
/// Never smaller, so the final bilinear step is always a reduction of under a
/// factor of two or an enlargement from full resolution — never a reduction
/// large enough to skip source pixels.
fn level_for(chain: &[Pixmap], target: IVec2) -> &Pixmap {
    chain
        .iter()
        .take_while(|l| l.width() as i32 >= target.x && l.height() as i32 >= target.y)
        .last()
        .or_else(|| chain.first())
        .expect("a chain always has its base level")
}

/// Scale, and if asked rotate, one motif into its own pixmap.
fn rasterise(
    chain: &[Pixmap],
    motif: &Motif,
    scale: f32,
    rotation_deg: f32,
    px_per_mm: f32,
) -> Result<Pixmap, RenderError> {
    let target = IVec2::new(
        ((motif.natural_size_mm.x * scale * px_per_mm).round() as i32).max(1),
        ((motif.natural_size_mm.y * scale * px_per_mm).round() as i32).max(1),
    );
    let base = level_for(chain, target);
    let sx = target.x as f32 / base.width() as f32;
    let sy = target.y as f32 / base.height() as f32;

    let paint = PixmapPaint {
        quality: FilterQuality::Bilinear,
        ..Default::default()
    };

    if rotation_deg == 0.0 {
        let mut out = new_pixmap(target)?;
        out.draw_pixmap(
            0,
            0,
            base.as_ref(),
            &paint,
            Transform::from_scale(sx, sy),
            None,
        );
        return Ok(out);
    }

    // Rotation widens the sprite; size the canvas to the rotated bounds so
    // nothing is clipped, and keep the motif centred within it.
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let (w, h) = (target.x as f32, target.y as f32);
    let bounds = IVec2::new(
        ((w * cos.abs() + h * sin.abs()).ceil() as i32).max(1),
        ((w * sin.abs() + h * cos.abs()).ceil() as i32).max(1),
    );
    let mut out = new_pixmap(bounds)?;
    let transform = Transform::from_translate(bounds.x as f32 / 2.0, bounds.y as f32 / 2.0)
        .pre_rotate(rotation_deg)
        .pre_translate(-w / 2.0, -h / 2.0)
        .pre_scale(sx, sy);
    out.draw_pixmap(0, 0, base.as_ref(), &paint, transform, None);
    Ok(out)
}

fn new_pixmap(size: IVec2) -> Result<Pixmap, RenderError> {
    u32::try_from(size.x)
        .ok()
        .zip(u32::try_from(size.y).ok())
        .and_then(|(w, h)| Pixmap::new(w, h))
        .ok_or(RenderError::Unallocatable {
            width: size.x,
            height: size.y,
        })
}

fn fill_ground(target: &mut Pixmap, pattern: &Pattern) -> Result<(), RenderError> {
    let Color { r, g, b } = pattern.palette.ground().ok_or(RenderError::NoGroundColor)?;
    // Opaque, so premultiplication is the identity here.
    let ground = PremultipliedColorU8::from_rgba(r, g, b, 255).ok_or(RenderError::NoGroundColor)?;
    for px in target.pixels_mut() {
        *px = ground;
    }
    Ok(())
}

/// Samples per pixel along each axis when drawing a ground.
///
/// Four means sixteen per pixel and seventeen levels of coverage along an
/// edge, which is past the point where a step is visible. It is a fixed cost
/// rather than an adaptive one on purpose: an early-out that decided a pixel
/// was uniform from a few samples would miss a feature finer than a pixel,
/// and a hairline is exactly what a designer reaches for.
const GROUND_SAMPLES: u8 = 4;

/// Lay a striped or checked ground over the fabric.
///
/// Between the ground and the motifs, which is the whole reason bands are a
/// property of the pattern rather than a kind of motif: a check with flowers
/// tossed over it is then the two things it looks like rather than a special
/// case.
///
/// Resolved per column and per row first and combined per pixel, because a
/// band sequence is one-dimensional in each direction — walking the sequence
/// for every pixel would do the same search a thousand times a row.
fn fill_bands(
    target: &mut Pixmap,
    pattern: &Pattern,
    pl: &PixelLattice,
) -> Result<(), RenderError> {
    let Some(ground) = &pattern.ground else {
        return Ok(());
    };
    if ground.is_empty() {
        return Ok(());
    }

    // Fitted to the tile, or whatever the ground lays down is cut at the
    // tile's edge and every join carries the remainder.
    let ground = laid_ground(ground, pattern.tile_size_mm.x, pattern.tile_size_mm.y);
    let (w, h) = (target.width(), target.height());
    let scale = Vec2::new(
        pattern.tile_size_mm.x / pl.tile_px.x.max(1) as f32,
        pattern.tile_size_mm.y / pl.tile_px.y.max(1) as f32,
    );

    // Sampled several times per pixel and averaged, because a ground is not
    // made of straight lines any more. One sample per pixel is right for a
    // stripe, whose edges are axis-aligned, and puts a staircase along every
    // curve of a Truchet arc or a grown blob — the more so beside the motifs,
    // which tiny-skia draws with filtering, so the two layers of the same
    // print did not look as though they had been made by the same tool.
    //
    // Reported from the app as the grounds looking "very pixelated", which is
    // the right description.
    //
    // Averaged in sRGB rather than in a linear space, to match how the motif
    // layer composites; the two blending differently would trade one visible
    // inconsistency for a subtler one.
    //
    // In the tile's own coordinates, not the canvas's. A field of nine tiles is
    // 720 mm across where a tile is 240, and an `f32` carries about six decimal
    // digits: asking for the slot at 683.1693 mm and at 3.1693 mm is the same
    // question but not the same arithmetic, and near a boundary the two answers
    // can differ. Taking the remainder in *pixels* first is exact, because a
    // field is always a whole number of tiles.
    //
    // Found by a grown ground disagreeing with itself in ninety pixels at 150
    // DPI. Bands had the same hazard and had never been unlucky enough to show
    // it.
    let tile_px = (pl.tile_px.x.max(1) as u32, pl.tile_px.y.max(1) as u32);
    let step = 1.0 / f32::from(GROUND_SAMPLES);
    let taken = f32::from(GROUND_SAMPLES) * f32::from(GROUND_SAMPLES);

    let pixels = target.pixels_mut();
    for y in 0..h {
        for x in 0..w {
            // Whatever is already here is what a sample landing on no part of
            // the ground should contribute — the fabric, in every order this
            // is called in today, and read rather than assumed so that it
            // stays right if it is ever called in another.
            let at = (y * w + x) as usize;
            let under = pixels[at].demultiply();
            let base = Color {
                r: under.red(),
                g: under.green(),
                b: under.blue(),
            };

            let (px, py) = ((x % tile_px.0) as f32, (y % tile_px.1) as f32);
            let mut sum = [0f32; 3];
            for j in 0..GROUND_SAMPLES {
                let y_mm = (py + (f32::from(j) + 0.5) * step) * scale.y;
                for i in 0..GROUND_SAMPLES {
                    let x_mm = (px + (f32::from(i) + 0.5) * step) * scale.x;
                    let Color { r, g, b } = ground
                        .slot_at(x_mm, y_mm)
                        .and_then(|slot| pattern.palette.resolve(slot))
                        .unwrap_or(base);
                    sum[0] += f32::from(r);
                    sum[1] += f32::from(g);
                    sum[2] += f32::from(b);
                }
            }

            let channel = |v: f32| (v / taken + 0.5) as u8;
            if let Some(p) = PremultipliedColorU8::from_rgba(
                channel(sum[0]),
                channel(sum[1]),
                channel(sum[2]),
                255,
            ) {
                pixels[at] = p;
            }
        }
    }
    Ok(())
}

/// Lay a ground on a tile, remembering the last one.
///
/// [`Ground::laid_over`] is a pure function of a ground and a tile size, and
/// for a grown ground it is the most expensive thing in a render — a few
/// hundred milliseconds where everything else is a few. A design session
/// renders the same ground over and over: a preview per control change, a
/// field and a tile, a proof, an export. Growing it again each time is work
/// with a known answer.
///
/// One entry, because the caller that matters is an interface showing one
/// pattern. A memo of a pure function changes nothing about what comes out;
/// the same ground and the same tile give the same field whether it was just
/// computed or is being handed back.
///
/// Public so that a caller which needs to *look at* the field — how much of
/// the tile each level covered, say — shares the memo with the render that
/// follows it rather than growing a second copy of the same thing.
#[must_use]
pub fn laid_ground(ground: &Ground, width_mm: f32, height_mm: f32) -> LaidGround {
    type Memo = Mutex<Option<(Ground, f32, f32, LaidGround)>>;
    static LAST: OnceLock<Memo> = OnceLock::new();

    let cell = LAST.get_or_init(|| Mutex::new(None));
    // A poisoned lock means another thread panicked mid-render. There is
    // nothing to recover and nothing to corrupt — the worst case is growing
    // the field again — so take the value through the poison rather than
    // propagating a panic out of a render.
    let mut held = cell.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((g, w, h, laid)) = held.as_ref() {
        if g == ground && *w == width_mm && *h == height_mm {
            return laid.clone();
        }
    }
    let laid = ground.laid_over(width_mm, height_mm);
    *held = Some((ground.clone(), width_mm, height_mm, laid.clone()));
    laid
}

/// Where a sprite's leading edge goes, given where its centre should land.
///
/// Pixel `j` covers the interval `[j, j+1)`, so reflecting a tile `T` pixels
/// wide sends pixel `j` to pixel `T-1-j`. Work that through a sprite of width
/// `w` starting at `c - floor(w/2)` and the reflected copy has to start at
/// `c' - ceil(w/2)`, not at `c' - floor(w/2)`. The difference is nothing for an
/// even width and one whole pixel for an odd one — enough to break the
/// symmetry a mirror repeat is entirely made of.
fn anchor(center: i32, extent: i32, flipped: bool) -> i32 {
    center
        - if flipped {
            extent - extent / 2
        } else {
            extent / 2
        }
}

/// A copy of the sprite reversed left to right.
///
/// Reversing indices is an exact reflection about the sprite's own centre —
/// no resampling, no filtering — which is what lets the mirrored quadrants of
/// a tile match the designed one pixel for pixel.
fn flipped_x(sprite: &Pixmap) -> Pixmap {
    let (w, h) = (sprite.width() as usize, sprite.height() as usize);
    let mut out = sprite.clone();
    let src = sprite.pixels();
    let dst = out.pixels_mut();
    for y in 0..h {
        for x in 0..w {
            dst[y * w + x] = src[y * w + (w - 1 - x)];
        }
    }
    out
}

/// A copy of the sprite reversed top to bottom.
fn flipped_y(sprite: &Pixmap) -> Pixmap {
    let (w, h) = (sprite.width() as usize, sprite.height() as usize);
    let mut out = sprite.clone();
    let src = sprite.pixels();
    let dst = out.pixels_mut();
    for y in 0..h {
        dst[y * w..(y + 1) * w].copy_from_slice(&src[(h - 1 - y) * w..(h - y) * w]);
    }
    out
}

/// Source-over compositing at a whole-pixel offset, clipped to the target.
///
/// Written out rather than delegated to a transform so that it is exactly a
/// copy: identical inputs at any offset give identical pixels, which is the
/// property the seamlessness test rests on.
fn blit(target: &mut Pixmap, src: &Pixmap, top_left: IVec2, clip_min: IVec2, clip_max: IVec2) {
    let (tw, th) = (target.width() as i32, target.height() as i32);
    let (sw, sh) = (src.width() as i32, src.height() as i32);

    let x0 = top_left.x.max(0).max(clip_min.x);
    let y0 = top_left.y.max(0).max(clip_min.y);
    let x1 = (top_left.x + sw).min(tw).min(clip_max.x);
    let y1 = (top_left.y + sh).min(th).min(clip_max.y);
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let src_px = src.pixels();
    let dst_px = target.pixels_mut();
    for y in y0..y1 {
        let sy = y - top_left.y;
        for x in x0..x1 {
            let sx = x - top_left.x;
            let s = src_px[(sy * sw + sx) as usize];
            if s.alpha() == 0 {
                continue;
            }
            let d = &mut dst_px[(y * tw + x) as usize];
            *d = if s.alpha() == 255 {
                s
            } else {
                source_over(s, *d)
            };
        }
    }
}

/// `src + dst * (1 - src.a)` on premultiplied 8-bit channels.
fn source_over(s: PremultipliedColorU8, d: PremultipliedColorU8) -> PremultipliedColorU8 {
    let inv = 255 - s.alpha() as u32;
    let mix = |sc: u8, dc: u8| -> u8 {
        let v = sc as u32 + div255(dc as u32 * inv);
        v.min(255) as u8
    };
    // Channels stay premultiplied and alpha is mixed the same way, so the
    // result is well-formed by construction and the unwrap cannot fire.
    PremultipliedColorU8::from_rgba(
        mix(s.red(), d.red()),
        mix(s.green(), d.green()),
        mix(s.blue(), d.blue()),
        mix(s.alpha(), d.alpha()),
    )
    .unwrap_or(s)
}

/// Rounded division by 255, the standard integer-compositing approximation.
fn div255(v: u32) -> u32 {
    let v = v + 128;
    (v + (v >> 8)) >> 8
}
