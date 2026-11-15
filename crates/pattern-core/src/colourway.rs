//! Deriving one palette from another.
//!
//! A colourway is the same pattern in a different palette, and textile ranges
//! sell in colourways, so producing candidates quickly is half of what the
//! tool is for. These are the three moves the spec names: rotate the hue, shift
//! the lightness, and swap figure for ground.
//!
//! All of it is arithmetic. Converting through HSL needs only comparison,
//! subtraction and division, so a derived colourway is exactly reproducible
//! anywhere, which is the same constraint that shaped the scatter sampler, honoured
//! here for the same reason.

use crate::palette::{Color, Palette, GROUND_SLOT};

/// Hue, saturation and lightness, each in `0..1`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Hsl {
    h: f32,
    s: f32,
    l: f32,
}

fn to_hsl(c: Color) -> Hsl {
    let (r, g, b) = (c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < 1e-6 {
        // Grey has no hue. Reporting zero rather than something arbitrary
        // matters: rotating a grey must leave it grey.
        return Hsl { h: 0.0, s: 0.0, l };
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    Hsl { h: h / 6.0, s, l }
}

fn from_hsl(hsl: Hsl) -> Color {
    let channel = |p: f32, q: f32, mut t: f32| -> f32 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };

    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if hsl.s <= 1e-6 {
        let v = byte(hsl.l);
        return Color::new(v, v, v);
    }

    let q = if hsl.l < 0.5 {
        hsl.l * (1.0 + hsl.s)
    } else {
        hsl.l + hsl.s - hsl.l * hsl.s
    };
    let p = 2.0 * hsl.l - q;
    Color::new(
        byte(channel(p, q, hsl.h + 1.0 / 3.0)),
        byte(channel(p, q, hsl.h)),
        byte(channel(p, q, hsl.h - 1.0 / 3.0)),
    )
}

/// Turn every colour the same distance around the wheel.
///
/// The cheapest way to four candidate colourways, and the one that keeps a
/// palette's internal relationships: everything moves together, so a print that
/// was harmonious stays harmonious.
#[must_use]
pub fn rotate_hue(palette: &Palette, degrees: f32) -> Palette {
    let turn = degrees / 360.0;
    Palette::new(
        palette
            .colors
            .iter()
            .map(|c| {
                let mut hsl = to_hsl(*c);
                hsl.h = (hsl.h + turn).rem_euclid(1.0);
                from_hsl(hsl)
            })
            .collect(),
    )
}

/// Move every colour toward white or black.
///
/// `amount` is a fraction: positive lightens, negative darkens. Applied as a
/// move *toward* the limit rather than an addition, so nothing clips: a
/// palette lightened repeatedly approaches white instead of flattening into it
/// at the first step.
#[must_use]
pub fn shift_lightness(palette: &Palette, amount: f32) -> Palette {
    Palette::new(
        palette
            .colors
            .iter()
            .map(|c| {
                let mut hsl = to_hsl(*c);
                hsl.l = if amount >= 0.0 {
                    hsl.l + (1.0 - hsl.l) * amount.min(1.0)
                } else {
                    hsl.l * (1.0 + amount.max(-1.0))
                };
                from_hsl(hsl)
            })
            .collect(),
    )
}

/// Exchange the ground with one of the figure colours.
///
/// Inverting a print to a dark ground costs nothing because the background is
/// slot 0 rather than a special case, which is exactly why it was made a slot
/// in the first place.
#[must_use]
pub fn swap_ground(palette: &Palette, slot: u8) -> Palette {
    let mut colors = palette.colors.clone();
    let index = slot as usize;
    if index < colors.len() && index != GROUND_SLOT as usize {
        colors.swap(GROUND_SLOT as usize, index);
    }
    Palette::new(colors)
}

/// Replace one slot's colour, leaving the rest alone.
#[must_use]
pub fn with_colour(palette: &Palette, slot: u8, colour: Color) -> Palette {
    let mut colors = palette.colors.clone();
    if let Some(existing) = colors.get_mut(slot as usize) {
        *existing = colour;
    }
    Palette::new(colors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn house() -> Palette {
        Palette::new(vec![
            Color::new(250, 246, 238),
            Color::new(200, 30, 45),
            Color::new(60, 120, 55),
        ])
    }

    #[test]
    fn a_colour_survives_a_round_trip_through_hsl() {
        for c in house().colors.iter().chain(&[
            Color::new(0, 0, 0),
            Color::new(255, 255, 255),
            Color::new(128, 128, 128),
            Color::new(7, 200, 13),
        ]) {
            let back = from_hsl(to_hsl(*c));
            let off = (back.r as i32 - c.r as i32)
                .abs()
                .max((back.g as i32 - c.g as i32).abs())
                .max((back.b as i32 - c.b as i32).abs());
            assert!(off <= 1, "{c:?} came back as {back:?}");
        }
    }

    #[test]
    fn a_full_turn_of_hue_changes_nothing() {
        let rotated = rotate_hue(&house(), 360.0);
        for (a, b) in house().colors.iter().zip(&rotated.colors) {
            let off = (a.r as i32 - b.r as i32)
                .abs()
                .max((a.g as i32 - b.g as i32).abs())
                .max((a.b as i32 - b.b as i32).abs());
            assert!(off <= 1, "{a:?} became {b:?}");
        }
    }

    #[test]
    fn rotating_moves_every_colour_the_same_way() {
        // The property that makes it useful: a harmonious palette stays
        // harmonious, because the relationships between its colours are
        // preserved.
        let rotated = rotate_hue(&house(), 120.0);
        let before: Vec<f32> = house().colors.iter().map(|c| to_hsl(*c).h).collect();
        let after: Vec<f32> = rotated.colors.iter().map(|c| to_hsl(*c).h).collect();
        // Slot 0 is nearly grey, so its hue is meaningless; compare the two
        // saturated ones.
        let gap = |v: &[f32]| (v[2] - v[1]).rem_euclid(1.0);
        assert!((gap(&before) - gap(&after)).abs() < 0.01);
    }

    #[test]
    fn rotating_leaves_a_grey_grey() {
        let greys = Palette::new(vec![Color::new(128, 128, 128), Color::new(20, 20, 20)]);
        let rotated = rotate_hue(&greys, 90.0);
        for c in &rotated.colors {
            assert_eq!(c.r, c.g, "{c:?} picked up a hue");
            assert_eq!(c.g, c.b, "{c:?} picked up a hue");
        }
    }

    #[test]
    fn lightening_approaches_white_without_clipping() {
        // Applied as a move toward the limit rather than an addition, so
        // repeated lightening keeps its structure instead of flattening to a
        // single value at the first step.
        let mut p = house();
        for _ in 0..5 {
            p = shift_lightness(&p, 0.3);
        }
        let distinct: std::collections::HashSet<(u8, u8, u8)> =
            p.colors.iter().map(|c| (c.r, c.g, c.b)).collect();
        assert_eq!(distinct.len(), 3, "the palette collapsed: {:?}", p.colors);
    }

    #[test]
    fn darkening_is_the_mirror_of_lightening() {
        let dark = shift_lightness(&house(), -0.5);
        for (a, b) in house().colors.iter().zip(&dark.colors) {
            assert!(
                to_hsl(*b).l <= to_hsl(*a).l + 1e-3,
                "{a:?} did not darken into {b:?}"
            );
        }
    }

    #[test]
    fn swapping_the_ground_inverts_a_print() {
        let inverted = swap_ground(&house(), 1);
        assert_eq!(inverted.ground(), Some(Color::new(200, 30, 45)));
        assert_eq!(inverted.resolve(1), Some(Color::new(250, 246, 238)));
        assert_eq!(inverted.resolve(2), Some(Color::new(60, 120, 55)));
    }

    #[test]
    fn swapping_the_ground_with_itself_does_nothing() {
        assert_eq!(swap_ground(&house(), 0), house());
        assert_eq!(swap_ground(&house(), 9), house());
    }

    #[test]
    fn a_derived_palette_keeps_its_length_so_slots_still_line_up() {
        // The whole mechanism depends on slot n meaning the same thing in
        // every colourway of a range.
        for derived in [
            rotate_hue(&house(), 40.0),
            shift_lightness(&house(), -0.2),
            swap_ground(&house(), 2),
            with_colour(&house(), 1, Color::new(1, 2, 3)),
        ] {
            assert_eq!(derived.len(), house().len());
        }
    }

    #[test]
    fn setting_one_colour_leaves_the_others_alone() {
        let edited = with_colour(&house(), 1, Color::new(10, 20, 30));
        assert_eq!(edited.resolve(1), Some(Color::new(10, 20, 30)));
        assert_eq!(edited.ground(), house().ground());
        assert_eq!(edited.resolve(2), house().resolve(2));
    }

    #[test]
    fn deriving_uses_no_transcendental_arithmetic() {
        // A colourway derived today must come back identical on another
        // machine, for the same reason a scatter must.
        let source = include_str!("colourway.rs");
        let body = source.split("#[cfg(test)]").next().expect("non-test part");
        for banned in [".sin(", ".cos(", ".atan2(", ".powf(", ".exp(", ".ln("] {
            assert!(
                !body.contains(banned),
                "{banned} would break reproducibility"
            );
        }
    }
}
