//! Colour conversion, against real ICC profiles.
//!
//! These use the profiles macOS ships, because a conversion tested against a
//! profile this project invented would prove only that the arithmetic runs.
//! They skip rather than fail where those are absent, so the suite still means
//! something on a machine without them.

use std::path::{Path, PathBuf};

use pattern_io::{Compression, Conversion, Expect, Export, Format, Profiles, Shape, Written};
use tiny_skia::{Paint, Pixmap, Rect, Transform};

const SRGB: &str = "/System/Library/ColorSync/Profiles/sRGB Profile.icc";
const CMYK: &str = "/System/Library/ColorSync/Profiles/Generic CMYK Profile.icc";

fn profiles() -> Option<(moxcms_profile::Profile, moxcms_profile::Profile)> {
    if !Path::new(SRGB).exists() || !Path::new(CMYK).exists() {
        return None;
    }
    Some((
        Profiles::load(Path::new(SRGB), Expect::Rgb).expect("sRGB should load"),
        Profiles::load(Path::new(CMYK), Expect::Cmyk).expect("CMYK should load"),
    ))
}

mod moxcms_profile {
    pub type Profile = moxcms::ColorProfile;
}

fn swatch(colour: [u8; 4]) -> Pixmap {
    let mut pixmap = Pixmap::new(8, 8).expect("allocation");
    let mut paint = Paint::default();
    paint.set_color_rgba8(colour[0], colour[1], colour[2], colour[3]);
    pixmap.fill_rect(
        Rect::from_xywh(0.0, 0.0, 8.0, 8.0).expect("rect"),
        &paint,
        Transform::identity(),
        None,
    );
    pixmap
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pattern-io-colour-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("scratch");
    dir.join(name)
}

#[test]
fn a_profile_of_the_wrong_kind_is_refused_by_name() {
    if !Path::new(SRGB).exists() {
        return;
    }
    // Handing a bureau's CMYK profile to the RGB slot is an easy mistake and a
    // silent one: the transform would still build and the colours would be
    // nonsense.
    let refused = Profiles::load(Path::new(SRGB), Expect::Cmyk);
    let message = format!("{}", refused.expect_err("an RGB profile is not a CMYK one"));
    assert!(message.contains("Rgb"), "{message}");
    assert!(message.contains("CMYK"), "{message}");
}

#[test]
fn something_that_is_not_a_profile_is_refused() {
    let path = scratch("nonsense.icc");
    std::fs::write(&path, b"this is not an ICC profile").expect("write");
    assert!(Profiles::load(&path, Expect::Rgb).is_err());
    let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
}

#[test]
fn ink_goes_where_ink_should_go() {
    // The conversion is only meaningful if the numbers mean something. White
    // takes almost no ink; black takes a lot; red is magenta and yellow.
    let Some((rgb, cmyk)) = profiles() else {
        return;
    };
    let conversion = Conversion::to_cmyk(&rgb, &cmyk, false).expect("transform");

    let white = conversion
        .pixmap_to_cmyk(&swatch([255, 255, 255, 255]))
        .expect("convert");
    assert!(
        white[0] < 12 && white[1] < 12 && white[2] < 12 && white[3] < 12,
        "white asked for ink: {:?}",
        &white[0..4]
    );

    let black = conversion
        .pixmap_to_cmyk(&swatch([0, 0, 0, 255]))
        .expect("convert");
    assert!(
        black[3] > 150,
        "black barely used the black plate: {:?}",
        &black[0..4]
    );

    let red = conversion
        .pixmap_to_cmyk(&swatch([200, 30, 45, 255]))
        .expect("convert");
    assert!(
        red[1] > red[0] && red[2] > red[0],
        "red should be magenta and yellow, not cyan: {:?}",
        &red[0..4]
    );
}

#[test]
fn a_conversion_produces_four_channels_for_every_pixel() {
    let Some((rgb, cmyk)) = profiles() else {
        return;
    };
    let conversion = Conversion::to_cmyk(&rgb, &cmyk, false).expect("transform");
    let pixmap = swatch([120, 60, 30, 255]);
    let converted = conversion.pixmap_to_cmyk(&pixmap).expect("convert");
    assert_eq!(converted.len(), pixmap.pixels().len() * 4);
}

#[test]
fn a_soft_proof_shows_what_the_press_cannot_reach() {
    // The reason soft proofing exists. A saturated screen colour survives the
    // round trip changed, and seeing it change beforehand is the difference
    // between a conversation and a reprint.
    let Some((rgb, cmyk)) = profiles() else {
        return;
    };
    let conversion = Conversion::to_cmyk(&rgb, &cmyk, true).expect("transform");

    let vivid = swatch([0, 200, 80, 255]);
    let proofed = conversion.soft_proof(&vivid).expect("proof");
    let before = vivid.pixels()[0].demultiply();
    let after = proofed.pixels()[0].demultiply();
    let moved = (before.red() as i32 - after.red() as i32).abs()
        + (before.green() as i32 - after.green() as i32).abs()
        + (before.blue() as i32 - after.blue() as i32).abs();
    assert!(moved > 10, "a vivid green came back unchanged: {moved}");

    // And something well inside the gamut should barely move, or the proof
    // would just be a filter.
    let muted = swatch([140, 120, 110, 255]);
    let proofed = conversion.soft_proof(&muted).expect("proof");
    let before = muted.pixels()[0].demultiply();
    let after = proofed.pixels()[0].demultiply();
    let moved = (before.red() as i32 - after.red() as i32).abs()
        + (before.green() as i32 - after.green() as i32).abs()
        + (before.blue() as i32 - after.blue() as i32).abs();
    assert!(moved < 40, "a muted colour moved a long way: {moved}");
}

#[test]
fn a_cmyk_tiff_carries_four_channels_and_its_profile() {
    // A CMYK file without its profile is a set of numbers nobody can
    // interpret, which is the failure colour management exists to prevent.
    let Some((rgb, cmyk)) = profiles() else {
        return;
    };
    let conversion = Conversion::to_cmyk(&rgb, &cmyk, false).expect("transform");
    let pixmap = swatch([200, 30, 45, 255]);
    let samples = conversion.pixmap_to_cmyk(&pixmap).expect("convert");
    let profile = std::fs::read(CMYK).expect("profile bytes");

    let path = scratch("cmyk.tif");
    pattern_io::write_written(
        &path,
        &pixmap,
        Written {
            export: Export {
                format: Format::Tiff,
                compression: Compression::None,
                dpi: 150.0,
                shape: Shape::Tile,
            },
            cmyk: Some((&samples, &profile)),
        },
    )
    .expect("should write");

    let file = std::fs::File::open(&path).expect("open");
    let mut decoder = tiff::decoder::Decoder::new(std::io::BufReader::new(file)).expect("decode");
    assert_eq!(decoder.dimensions().expect("dimensions"), (8, 8));

    let photometric = decoder
        .get_tag(tiff::tags::Tag::PhotometricInterpretation)
        .expect("photometric")
        .into_u32()
        .expect("number");
    assert_eq!(photometric, 5, "a CMYK TIFF declares photometric 5");

    let embedded = decoder
        .get_tag(tiff::tags::Tag::Unknown(34675))
        .expect("the profile should be embedded")
        .into_u8_vec()
        .expect("bytes");
    assert_eq!(embedded.len(), profile.len(), "the profile was truncated");
    let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
}
