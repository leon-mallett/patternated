use glam::Vec2;

use crate::{Format, Shape};

/// A filename that says what the file is.
///
/// `peelclub-cherries-halfdrop-240mm-150dpi.tif`. The spec calls this out as
/// removing an entire category of mistake, and it is right: a print file's
/// repeat size and resolution are invisible once it is on somebody else's
/// machine, and a bureau given `final_v3.tif` has to ask or guess.
///
/// A pre-repeated sheet says so and gives its tile count, because "240mm" on a
/// sheet would otherwise be read as the size of the whole file.
#[must_use]
pub fn suggested_name(
    stem: &str,
    lattice: &str,
    tile_size_mm: Vec2,
    dpi: f32,
    shape: Shape,
    format: Format,
) -> String {
    let slug = |text: &str| {
        let cleaned: String = text
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let trimmed: Vec<&str> = cleaned.split('-').filter(|p| !p.is_empty()).collect();
        trimmed.join("-").to_lowercase()
    };

    let mut parts = vec![slug(stem)];
    let lattice = slug(lattice);
    if !lattice.is_empty() {
        parts.push(lattice.replace('-', ""));
    }
    if let Shape::Sheet { min_cols, min_rows } = shape {
        parts.push(format!("{min_cols}x{min_rows}sheet"));
    }
    parts.push(format!(
        "{}x{}mm",
        round_mm(tile_size_mm.x),
        round_mm(tile_size_mm.y)
    ));
    parts.push(format!("{}dpi", dpi.round() as i64));

    let stem: String = parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    format!("{stem}.{}", format.extension())
}

/// Millimetres as a whole number where it is one, and one decimal otherwise.
fn round_mm(value: f32) -> String {
    if (value - value.round()).abs() < 0.05 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_is_named_for_its_repeat_and_resolution() {
        assert_eq!(
            suggested_name(
                "Peel Club cherries",
                "half-drop",
                Vec2::new(240.0, 180.0),
                150.0,
                Shape::Tile,
                Format::Tiff
            ),
            "peel-club-cherries-halfdrop-240x180mm-150dpi.tif"
        );
    }

    #[test]
    fn a_sheet_says_it_is_a_sheet() {
        // Otherwise the repeat size reads as the size of the whole file.
        assert_eq!(
            suggested_name(
                "cherries",
                "block",
                Vec2::new(240.0, 180.0),
                300.0,
                Shape::Sheet {
                    min_cols: 4,
                    min_rows: 3
                },
                Format::Png
            ),
            "cherries-block-4x3sheet-240x180mm-300dpi.png"
        );
    }

    #[test]
    fn awkward_names_survive_becoming_a_filename() {
        let name = suggested_name(
            "Spring/Summer 27 — \"cherries\"",
            "half-drop",
            Vec2::new(240.0, 180.0),
            150.0,
            Shape::Tile,
            Format::Tiff,
        );
        assert_eq!(
            name,
            "spring-summer-27-cherries-halfdrop-240x180mm-150dpi.tif"
        );
        assert!(!name.contains("--"), "{name} has an empty part in it");
    }

    #[test]
    fn a_fractional_repeat_keeps_its_decimal() {
        // 127 mm repeats exist; so do 127.5 mm ones, and rounding the name
        // would make two different designs look like the same file.
        let name = suggested_name(
            "cherries",
            "block",
            Vec2::new(127.5, 90.0),
            150.0,
            Shape::Tile,
            Format::Tiff,
        );
        assert!(name.contains("127.5x90mm"), "{name}");
    }
}
