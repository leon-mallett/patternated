//! Rasterising a [`Pattern`] to pixels at any resolution.
//!
//! Two ways to build a multi-tile field are offered, and the fact that they
//! agree pixel for pixel is what proves the repeat is seamless. See
//! [`render_field`] and [`render_field_direct`].

pub mod mockup;
pub mod motif;
mod pixel_lattice;

use glam::IVec2;
use pattern_core::{wrap_into_tile, Color, Motif, Pattern, PatternError};
use thiserror::Error;
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, PremultipliedColorU8, Transform};

pub use mockup::{render_mockup, Garment, Mockup};
pub use motif::{resolve, MotifBank};
pub use pixel_lattice::PixelLattice;

/// Ways a render can fail.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error(transparent)]
    Pattern(#[from] PatternError),

    #[error("placement {index} refers to motif '{motif_id}', which is not in the bank")]
    UnknownMotif { index: usize, motif_id: String },

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

    for (sprite, center_px) in sprites(pattern, bank, dpi)? {
        for offset in pl.neighbour_offsets() {
            blit_centered(&mut target, &sprite, center_px + offset);
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
        blit(&mut target, &tile, offset);
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

    let offsets = pl.offsets_covering(size.x, size.y);
    for (sprite, center_px) in sprites(pattern, bank, dpi)? {
        for offset in &offsets {
            blit_centered(&mut target, &sprite, center_px + *offset);
        }
    }
    Ok(target)
}

/// The quantised lattice a render will use at this resolution.
pub fn pixel_lattice_for(pattern: &Pattern, dpi: f32) -> Result<PixelLattice, RenderError> {
    if dpi <= 0.0 {
        return Err(RenderError::NonPositiveDpi(dpi));
    }
    Ok(PixelLattice::new(
        &pattern.lattice,
        pattern.tile_size_mm,
        dpi,
    ))
}

/// Rasterise each placement once, to be blitted wherever it is needed.
///
/// Every copy of a placement is then the same pixels at a whole-pixel offset.
/// That is what makes a tile and a directly drawn field agree exactly: were
/// each copy resampled at its own sub-pixel position, two copies of one motif
/// would differ slightly and the seam would show under inspection.
///
/// The cost is that positions quantise to whole pixels — 0.17 mm at 150 DPI,
/// which no fabric records.
fn sprites(
    pattern: &Pattern,
    bank: &MotifBank,
    dpi: f32,
) -> Result<Vec<(Pixmap, IVec2)>, RenderError> {
    let k = pattern_core::px_per_mm(dpi);

    // Resolve each distinct motif once. A pattern is usually many placements of
    // a few motifs, so resolving per placement would redo the same slot lookup
    // for every copy on the tile.
    let mut resolved: std::collections::HashMap<&str, Pixmap> = std::collections::HashMap::new();
    for placement in &pattern.placements {
        if resolved.contains_key(placement.motif_id.as_str()) {
            continue;
        }
        let motif = lookup(bank, pattern, &placement.motif_id)?;
        resolved.insert(
            placement.motif_id.as_str(),
            motif::resolve(motif, &pattern.palette)?,
        );
    }

    let mut out = Vec::with_capacity(pattern.placements.len());
    for placement in &pattern.placements {
        let motif = lookup(bank, pattern, &placement.motif_id)?;
        let base = resolved
            .get(placement.motif_id.as_str())
            .expect("every motif was resolved above");
        let sprite = rasterise(base, motif, placement.scale, placement.rotation_deg, k)?;
        let center_mm = wrap_into_tile(placement.position_mm, pattern.tile_size_mm);
        out.push((
            sprite,
            IVec2::new(
                (center_mm.x * k).round() as i32,
                (center_mm.y * k).round() as i32,
            ),
        ));
    }
    Ok(out)
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

/// Scale, and if asked rotate, one motif into its own pixmap.
fn rasterise(
    base: &Pixmap,
    motif: &Motif,
    scale: f32,
    rotation_deg: f32,
    px_per_mm: f32,
) -> Result<Pixmap, RenderError> {
    let target = IVec2::new(
        ((motif.natural_size_mm.x * scale * px_per_mm).round() as i32).max(1),
        ((motif.natural_size_mm.y * scale * px_per_mm).round() as i32).max(1),
    );
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

/// Blit a sprite so that its centre lands on `center_px`.
fn blit_centered(target: &mut Pixmap, sprite: &Pixmap, center_px: IVec2) {
    let top_left = IVec2::new(
        center_px.x - sprite.width() as i32 / 2,
        center_px.y - sprite.height() as i32 / 2,
    );
    blit(target, sprite, top_left);
}

/// Source-over compositing at a whole-pixel offset, clipped to the target.
///
/// Written out rather than delegated to a transform so that it is exactly a
/// copy: identical inputs at any offset give identical pixels, which is the
/// property the seamlessness test rests on.
fn blit(target: &mut Pixmap, src: &Pixmap, top_left: IVec2) {
    let (tw, th) = (target.width() as i32, target.height() as i32);
    let (sw, sh) = (src.width() as i32, src.height() as i32);

    let x0 = top_left.x.max(0);
    let y0 = top_left.y.max(0);
    let x1 = (top_left.x + sw).min(tw);
    let y1 = (top_left.y + sh).min(th);
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
