//! Converting a rendered pattern into the colours a press will actually lay
//! down.
//!
//! "It looked different on screen" is the commonest complaint in fabric
//! printing and is almost always an unmanaged conversion: a formula applied
//! instead of a profile. So there is no formula here. Everything goes through
//! an ICC profile, and the profile is a file somebody chose.

use std::path::Path;

use moxcms::{ColorProfile, Layout, TransformOptions};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tiny_skia::Pixmap;

#[derive(Debug, Error)]
pub enum ColourError {
    #[error("could not read the profile at {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} is not a usable ICC profile: {reason}")]
    Parse { path: String, reason: String },

    #[error("{path} describes a {found} profile, and a {wanted} one is needed here")]
    WrongSpace {
        path: String,
        found: String,
        wanted: String,
    },

    #[error("could not build a colour transform: {0}")]
    Transform(String),
}

/// Which colours an export is written in.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ColourSpace {
    /// Straight RGB, in whatever space the artwork was authored in.
    ///
    /// The right answer surprisingly often: many digital textile bureaux prefer
    /// RGB and do their own separation, because they know their press and you
    /// do not.
    #[default]
    Rgb,
    /// CMYK, converted through a named profile.
    ///
    /// The profile is a path rather than a bundled default on purpose; see
    /// [`Profiles`].
    Cmyk { profile: String },
}

/// A loaded pair of profiles and the transform between them.
pub struct Conversion {
    transform: std::sync::Arc<dyn moxcms::TransformExecutor<u8> + Send + Sync>,
    /// The reverse, for showing on screen what the press will do.
    proof: Option<std::sync::Arc<dyn moxcms::TransformExecutor<u8> + Send + Sync>>,
}

/// Where the profiles come from.
///
/// **Nothing is bundled.** Every CMYK profile that is free to redistribute,
/// meaning the ECI and IDEAlliance ones, describes an *offset litho* press, and a
/// digital textile press is not one. Shipping one as a default would produce a
/// file that looks converted and is wrong, which is worse than producing
/// nothing: the whole point of colour management is that the numbers mean
/// something.
///
/// So a profile is supplied. The bureau has one; it is the only correct answer;
/// and asking for it is a one-line email rather than a guess.
pub struct Profiles;

impl Profiles {
    /// Load a profile from disk, checking it describes the space expected.
    pub fn load(path: &Path, wanted: Expect) -> Result<ColorProfile, ColourError> {
        let bytes = std::fs::read(path).map_err(|source| ColourError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let profile = ColorProfile::new_from_slice(&bytes).map_err(|e| ColourError::Parse {
            path: path.display().to_string(),
            reason: format!("{e:?}"),
        })?;

        let found = profile.color_space;
        let ok = match wanted {
            Expect::Rgb => found == moxcms::DataColorSpace::Rgb,
            Expect::Cmyk => found == moxcms::DataColorSpace::Cmyk,
        };
        if !ok {
            return Err(ColourError::WrongSpace {
                path: path.display().to_string(),
                found: format!("{found:?}"),
                wanted: match wanted {
                    Expect::Rgb => "RGB",
                    Expect::Cmyk => "CMYK",
                }
                .into(),
            });
        }
        Ok(profile)
    }
}

/// What a profile is expected to describe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    Rgb,
    Cmyk,
}

impl Conversion {
    /// Build a conversion from an RGB profile to a CMYK one.
    ///
    /// `proof` also builds the return journey, which is what a soft proof is:
    /// the colours a press can reach, shown on a screen that can reach more.
    pub fn to_cmyk(
        source: &ColorProfile,
        destination: &ColorProfile,
        proof: bool,
    ) -> Result<Self, ColourError> {
        // CMYK is four channels and moxcms lays it out like RGBA.
        let transform = source
            .create_transform_8bit(
                Layout::Rgb,
                destination,
                Layout::Rgba,
                TransformOptions::default(),
            )
            .map_err(|e| ColourError::Transform(format!("{e:?}")))?;

        let back = if proof {
            Some(
                destination
                    .create_transform_8bit(
                        Layout::Rgba,
                        source,
                        Layout::Rgb,
                        TransformOptions::default(),
                    )
                    .map_err(|e| ColourError::Transform(format!("{e:?}")))?,
            )
        } else {
            None
        };

        Ok(Self {
            transform,
            proof: back,
        })
    }

    /// Convert a rendered pattern to interleaved CMYK bytes.
    pub fn pixmap_to_cmyk(&self, pixmap: &Pixmap) -> Result<Vec<u8>, ColourError> {
        let mut rgb = Vec::with_capacity(pixmap.pixels().len() * 3);
        for px in pixmap.pixels() {
            let c = px.demultiply();
            rgb.extend_from_slice(&[c.red(), c.green(), c.blue()]);
        }
        let mut cmyk = vec![0u8; pixmap.pixels().len() * 4];
        self.transform
            .transform(&rgb, &mut cmyk)
            .map_err(|e| ColourError::Transform(format!("{e:?}")))?;
        Ok(cmyk)
    }

    /// Show on screen what the press will do.
    ///
    /// Converts to the press's colours and straight back. Anything the press
    /// cannot reach comes back changed, which is the entire point: a designer
    /// seeing a flat, dulled red before the fabric arrives can still do
    /// something about it.
    pub fn soft_proof(&self, pixmap: &Pixmap) -> Result<Pixmap, ColourError> {
        let Some(back) = &self.proof else {
            return Ok(pixmap.clone());
        };
        let cmyk = self.pixmap_to_cmyk(pixmap)?;
        let mut rgb = vec![0u8; pixmap.pixels().len() * 3];
        back.transform(&cmyk, &mut rgb)
            .map_err(|e| ColourError::Transform(format!("{e:?}")))?;

        let mut out = pixmap.clone();
        for (px, chunk) in out.pixels_mut().iter_mut().zip(rgb.chunks_exact(3)) {
            // The render is opaque throughout, since the ground is a printed
            // colour, so premultiplication is the identity here.
            *px = tiny_skia::PremultipliedColorU8::from_rgba(chunk[0], chunk[1], chunk[2], 255)
                .unwrap_or(*px);
        }
        Ok(out)
    }
}
