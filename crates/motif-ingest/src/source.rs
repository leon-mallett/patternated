use serde::{Deserialize, Serialize};

/// Straight (non-premultiplied) RGBA pixels, row-major.
///
/// The crate's input type, and the reason it needs no image-format dependency
/// at all: decoding a PNG, a JPEG or a TIFF is the caller's problem, which
/// keeps format support a decision made at the edge rather than a dependency
/// baked into the pipeline.
///
/// Straight rather than premultiplied because matting and quantisation both
/// reason about a pixel's colour independently of its coverage, and
/// premultiplied colour is meaningless where alpha is zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceImage {
    pub width: u32,
    pub height: u32,
    /// Four bytes per pixel: red, green, blue, alpha.
    pub pixels: Vec<u8>,
}

impl SourceImage {
    #[must_use]
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        if pixels.len() != width as usize * height as usize * 4 {
            return None;
        }
        Some(Self {
            width,
            height,
            pixels,
        })
    }

    /// An opaque image of one colour, mostly useful for tests and for a
    /// generated motif that arrived without an alpha channel.
    #[must_use]
    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        Self {
            width,
            height,
            pixels: rgba.repeat(width as usize * height as usize),
        }
    }

    #[must_use]
    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        self.pixels[i..i + 4].copy_from_slice(&rgba);
    }

    /// A stable digest of the pixels, for noticing that a source has changed.
    ///
    /// FNV-1a, written out rather than taken from a crate, because this value
    /// is stored on disk and compared across runs: `DefaultHasher` makes no
    /// promise of stability between Rust releases, so a motif would appear to
    /// have changed after a toolchain upgrade. Not a security hash and not used
    /// as one — it answers "is this the same artwork as last time", and the
    /// consequence of a collision is a version that should have been created
    /// and was not.
    #[must_use]
    pub fn digest(&self) -> String {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x1000_0000_01b3;
        let mut hash = OFFSET;
        for byte in self
            .width
            .to_le_bytes()
            .iter()
            .chain(self.height.to_le_bytes().iter())
            .chain(self.pixels.iter())
        {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(PRIME);
        }
        format!("{hash:016x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mismatched_buffer_is_refused() {
        assert!(SourceImage::new(2, 2, vec![0; 10]).is_none());
        assert!(SourceImage::new(2, 2, vec![0; 16]).is_some());
    }

    #[test]
    fn the_digest_changes_with_the_pixels() {
        let a = SourceImage::filled(4, 4, [255, 0, 0, 255]);
        let mut b = a.clone();
        b.set_pixel(2, 2, [255, 0, 1, 255]);
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn the_digest_changes_with_the_shape() {
        // Same bytes, different dimensions, so the same digest would be wrong.
        let a = SourceImage::filled(4, 1, [1, 2, 3, 4]);
        let b = SourceImage::filled(1, 4, [1, 2, 3, 4]);
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn the_digest_is_stable_for_identical_input() {
        let a = SourceImage::filled(8, 8, [12, 34, 56, 255]);
        assert_eq!(a.digest(), a.clone().digest());
    }
}
