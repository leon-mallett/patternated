use glam::Vec2;
use serde::{Deserialize, Serialize};

use pattern_core::MM_PER_INCH;

/// Width of a standard apparel or home-textile fabric roll.
///
/// Most digital textile printers run 1.6 to 1.8 m, which is what most rolls
/// are. A repeat wider than this is usually a mistake rather than a choice.
pub const STANDARD_ROLL_MM: f32 = 1600.0;

/// Where "grand format" begins.
///
/// Wide-format machines reach 3.2 m and industrial roll-to-roll goes to 5 m,
/// but past this point the number of printers that can take the job drops
/// sharply. Erring high on purpose: the limit belongs to a specific printer and
/// telling somebody their design is too wide when it is not would be worse than
/// saying nothing.
pub const GRAND_FORMAT_MM: f32 = 3200.0;

/// What an export will physically be.
///
/// The engine works in millimetres and resolves pixels only here, so this is
/// where a designer finds out that their 240 mm repeat is a 1417 px file and,
/// more usefully, whether the thing they are about to export can be printed at
/// all.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PrintSize {
    pub width_mm: f32,
    pub height_mm: f32,
    pub width_px: u32,
    pub height_px: u32,
    pub dpi: f32,
}

/// How an export's width compares with what a printer is likely to take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidthVerdict {
    /// Within a standard roll.
    Fits,
    /// Past a standard roll but within wide format. Printable, by fewer people.
    WiderThanStandardRoll,
    /// Past grand format. Almost certainly not printable as one piece.
    WiderThanGrandFormat,
}

impl PrintSize {
    #[must_use]
    pub fn new(size_mm: Vec2, dpi: f32) -> Self {
        let scale = dpi / MM_PER_INCH;
        Self {
            width_mm: size_mm.x,
            height_mm: size_mm.y,
            width_px: ((size_mm.x * scale).round() as u32).max(1),
            height_px: ((size_mm.y * scale).round() as u32).max(1),
            dpi,
        }
    }

    /// The size of a file that already exists.
    ///
    /// Distinct from [`PrintSize::new`], which computes pixels from a design.
    /// Once a pattern has been rendered, the pixel count is the fact and the
    /// physical size follows from it. Quantising the tile to a whole number of
    /// pixels, and to a number its lattice divides, moves the repeat by a
    /// fraction of a millimetre, and a readout that reported the nominal size
    /// instead would disagree with the file it describes.
    #[must_use]
    pub fn from_pixels(width_px: u32, height_px: u32, dpi: f32) -> Self {
        let scale = MM_PER_INCH / dpi.max(1e-3);
        Self {
            width_mm: width_px as f32 * scale,
            height_mm: height_px as f32 * scale,
            width_px,
            height_px,
            dpi,
        }
    }

    /// Whether this is likely to be printable as one piece.
    ///
    /// Against a roll width that defaults to a standard one but is a setting,
    /// because the real limit belongs to a specific printer and nobody has
    /// picked one yet.
    #[must_use]
    pub fn width_verdict(&self, roll_width_mm: f32) -> WidthVerdict {
        if self.width_mm <= roll_width_mm {
            WidthVerdict::Fits
        } else if self.width_mm <= GRAND_FORMAT_MM {
            WidthVerdict::WiderThanStandardRoll
        } else {
            WidthVerdict::WiderThanGrandFormat
        }
    }

    /// Longest edge in metres, for saying the size out loud.
    #[must_use]
    pub fn longest_edge_m(&self) -> f32 {
        self.width_mm.max(self.height_mm) / 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_follow_from_millimetres_and_resolution() {
        // 240 mm at 150 DPI is 1417.32 px. The file is 1417; the design stays
        // 240 mm.
        let s = PrintSize::new(Vec2::new(240.0, 180.0), 150.0);
        assert_eq!((s.width_px, s.height_px), (1417, 1063));
        assert_eq!(s.width_mm, 240.0);
    }

    #[test]
    fn resolution_changes_the_file_and_not_the_design() {
        let a = PrintSize::new(Vec2::new(240.0, 180.0), 150.0);
        let b = PrintSize::new(Vec2::new(240.0, 180.0), 300.0);
        assert_eq!(b.width_px, 2835);
        assert_eq!(a.width_mm, b.width_mm);
    }

    #[test]
    fn a_rendered_file_reports_its_own_size() {
        // A 240 x 180 mm repeat at 150 DPI does not land on whole pixels, and
        // a half-drop rounds its height again so the lattice divides it. The
        // readout must describe the file rather than the intention.
        let s = PrintSize::from_pixels(1417, 1064, 150.0);
        assert!((s.width_mm - 239.9).abs() < 0.1, "{}", s.width_mm);
        assert!((s.height_mm - 180.2).abs() < 0.1, "{}", s.height_mm);
        assert_eq!((s.width_px, s.height_px), (1417, 1064));
    }

    #[test]
    fn a_normal_repeat_fits_a_roll() {
        let s = PrintSize::new(Vec2::new(240.0, 180.0), 150.0);
        assert_eq!(s.width_verdict(STANDARD_ROLL_MM), WidthVerdict::Fits);
    }

    #[test]
    fn a_wide_sheet_is_flagged_without_being_refused() {
        // A pre-repeated sheet grows quickly: eight tiles of 240 mm is 1.92 m,
        // past a standard roll and still printable by a wide-format machine.
        let s = PrintSize::new(Vec2::new(1920.0, 540.0), 150.0);
        assert_eq!(
            s.width_verdict(STANDARD_ROLL_MM),
            WidthVerdict::WiderThanStandardRoll
        );
    }

    #[test]
    fn past_grand_format_is_a_different_answer() {
        let s = PrintSize::new(Vec2::new(4000.0, 540.0), 150.0);
        assert_eq!(
            s.width_verdict(STANDARD_ROLL_MM),
            WidthVerdict::WiderThanGrandFormat
        );
    }

    #[test]
    fn the_roll_width_is_a_setting_not_a_rule() {
        // The real limit belongs to a specific printer. A narrow roll should
        // flag a repeat that a standard one would take.
        let s = PrintSize::new(Vec2::new(1400.0, 500.0), 150.0);
        assert_eq!(s.width_verdict(STANDARD_ROLL_MM), WidthVerdict::Fits);
        assert_eq!(s.width_verdict(1100.0), WidthVerdict::WiderThanStandardRoll);
    }
}
