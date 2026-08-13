//! The icon pipeline: one PNG in, three platform formats out.
//!
//! Arguably the highest-value component of the tool, because it is the part
//! nobody wants to do by hand. Windows wants a multi-resolution `.ico`, macOS
//! wants an `.icns`, and freedesktop wants eight loose PNGs in a directory tree
//! whose paths encode the size. All three are derived from a single square PNG.
//!
//! Like everything in core, this takes **bytes**, not a path.

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, RgbaImage};

use crate::error::{Error, Result};

/// Sizes embedded in the generated `.ico`. 256 is stored as PNG inside the ICO
/// container (Vista+), which the `ico` crate handles.
pub const ICO_SIZES: &[u32] = &[16, 32, 48, 64, 128, 256];

/// Sizes offered to the `.icns` family. Only sizes Apple defines an icon type
/// for are attempted; anything the encoder rejects is reported as a warning
/// rather than failing the build.
pub const ICNS_SIZES: &[u32] = &[16, 32, 64, 128, 256, 512, 1024];

/// The sizes the freedesktop hicolor theme expects for a MIME icon.
pub const HICOLOR_SIZES: &[u32] = &[16, 22, 24, 32, 48, 64, 128, 256];

/// Below this, the 256px entries stop being downscales and start being
/// upscales. Not a hard floor — a 128px source still produces every format,
/// just softly — so it is a warning, not an error.
pub const RECOMMENDED_MIN: u32 = 512;

/// Something about the source image the user should know. Never fatal — a
/// pipeline that refuses to run because the icon is 480px instead of 512px is
/// a pipeline people stop using.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconWarning {
    /// Source is not square; it was letterboxed rather than cropped.
    NotSquare { width: u32, height: u32 },
    /// Source is smaller than [`RECOMMENDED_MIN`].
    Small { size: u32 },
    /// A requested size was larger than the source, so it was upscaled.
    Upscaled { size: u32 },
    /// The source has no transparency at all, which usually means a flattened
    /// export and a white box on dark themes.
    Opaque,
    /// An `.icns` entry could not be encoded at this size.
    IcnsSizeSkipped { size: u32, reason: String },
}

impl std::fmt::Display for IconWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IconWarning::NotSquare { width, height } => write!(
                f,
                "source icon is {width}×{height}, not square — it was padded with transparency, \
                 not cropped; supply a square PNG to control the framing yourself"
            ),
            IconWarning::Small { size } => write!(
                f,
                "source icon is {size}×{size}; {RECOMMENDED_MIN}×{RECOMMENDED_MIN} or larger is \
                 recommended so the 256px entries are downscales"
            ),
            IconWarning::Upscaled { size } => {
                write!(
                    f,
                    "the {size}×{size} entry was upscaled from a smaller source and will look soft"
                )
            }
            IconWarning::Opaque => write!(
                f,
                "source icon has no transparent pixels; it will render as a solid rectangle in \
                 file managers"
            ),
            IconWarning::IcnsSizeSkipped { size, reason } => {
                write!(f, "skipped the {size}×{size} .icns entry: {reason}")
            }
        }
    }
}

/// Every icon format, generated in memory.
#[derive(Debug, Clone)]
pub struct IconSet {
    /// Multi-resolution Windows icon.
    pub ico: Vec<u8>,
    /// macOS icon family.
    pub icns: Vec<u8>,
    /// `(size, png_bytes)` for the hicolor tree, ascending.
    pub hicolor: Vec<(u32, Vec<u8>)>,
    /// Dimensions of the source, before squaring.
    pub source_size: (u32, u32),
    pub warnings: Vec<IconWarning>,
}

impl IconSet {
    /// Decode a PNG and render every derived format.
    ///
    /// Resampling is Lanczos3 in both directions. Alpha is preserved
    /// throughout: the source is converted to RGBA8 once and every derivative
    /// comes from that buffer.
    pub fn from_png_bytes(bytes: &[u8]) -> Result<IconSet> {
        if bytes.is_empty() {
            return Err(Error::Icon("icon file is empty".into()));
        }
        let img = image::load_from_memory_with_format(bytes, ImageFormat::Png)
            .map_err(|e| Error::Icon(format!("could not decode PNG: {e}")))?;

        let mut warnings = Vec::new();
        let (w, h) = (img.width(), img.height());
        if w == 0 || h == 0 {
            return Err(Error::Icon("icon has zero width or height".into()));
        }

        let mut rgba = img.into_rgba8();

        if w != h {
            warnings.push(IconWarning::NotSquare {
                width: w,
                height: h,
            });
            rgba = pad_to_square(&rgba);
        }
        let side = rgba.width();

        if side < RECOMMENDED_MIN {
            warnings.push(IconWarning::Small { size: side });
        }

        if rgba.pixels().all(|p| p.0[3] == 255) {
            warnings.push(IconWarning::Opaque);
        }

        let source = DynamicImage::ImageRgba8(rgba);

        // Every target size, resampled once and shared between the three
        // encoders. Lanczos3 for downscale as specified; also used upward,
        // where it is at least honest about being soft.
        let mut needed: Vec<u32> = ICO_SIZES
            .iter()
            .chain(ICNS_SIZES)
            .chain(HICOLOR_SIZES)
            .copied()
            .collect();
        needed.sort_unstable();
        needed.dedup();

        let mut scaled: Vec<(u32, RgbaImage)> = Vec::with_capacity(needed.len());
        for &size in &needed {
            if size > side {
                warnings.push(IconWarning::Upscaled { size });
            }
            let resized = if size == side {
                source.to_rgba8()
            } else {
                source
                    .resize_exact(size, size, FilterType::Lanczos3)
                    .to_rgba8()
            };
            scaled.push((size, resized));
        }
        let at = |size: u32| -> &RgbaImage {
            &scaled
                .iter()
                .find(|(s, _)| *s == size)
                .expect("every requested size was rendered")
                .1
        };

        let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
        for &size in ICO_SIZES {
            let img = at(size);
            let entry = ico::IconImage::from_rgba_data(size, size, img.as_raw().clone());
            let encoded = ico::IconDirEntry::encode(&entry)
                .map_err(|e| Error::Icon(format!("could not encode {size}px ICO entry: {e}")))?;
            dir.add_entry(encoded);
        }
        let mut ico_bytes = Vec::new();
        dir.write(&mut ico_bytes)
            .map_err(|e| Error::Icon(format!("could not write ICO: {e}")))?;

        // Two passes. The first adds the standard 1x entry for each size,
        // letting the encoder pick the best-supported container for it (the
        // legacy RLE types are still the safest choice at 16 and 32). The
        // second adds the Retina @2x entries, which `add_icon` will not infer
        // because a 64×64 buffer is equally plausibly `icp6` or `ic12`. Without
        // them macOS upscales the 1x art on every Retina display, which is the
        // one place a fuzzy icon is most obvious.
        let mut family = icns::IconFamily::new();
        for &size in ICNS_SIZES {
            let img = at(size);
            let icns_img = match icns::Image::from_data(
                icns::PixelFormat::RGBA,
                size,
                size,
                img.as_raw().clone(),
            ) {
                Ok(i) => i,
                Err(e) => {
                    warnings.push(IconWarning::IcnsSizeSkipped {
                        size,
                        reason: e.to_string(),
                    });
                    continue;
                }
            };
            if let Err(e) = family.add_icon(&icns_img) {
                warnings.push(IconWarning::IcnsSizeSkipped {
                    size,
                    reason: e.to_string(),
                });
                continue;
            }
            if let Some(retina) = icns::IconType::from_pixel_size_and_density(size, size, 2) {
                if !family.has_icon_with_type(retina) {
                    if let Err(e) = family.add_icon_with_type(&icns_img, retina) {
                        warnings.push(IconWarning::IcnsSizeSkipped {
                            size,
                            reason: e.to_string(),
                        });
                    }
                }
            }
        }
        let mut icns_bytes = Vec::new();
        if family.is_empty() {
            return Err(Error::Icon(
                "no .icns entries could be encoded from this source".into(),
            ));
        }
        family
            .write(&mut icns_bytes)
            .map_err(|e| Error::Icon(format!("could not write ICNS: {e}")))?;

        let mut hicolor = Vec::with_capacity(HICOLOR_SIZES.len());
        for &size in HICOLOR_SIZES {
            hicolor.push((size, encode_png(at(size))?));
        }

        Ok(IconSet {
            ico: ico_bytes,
            icns: icns_bytes,
            hicolor,
            source_size: (w, h),
            warnings,
        })
    }

    /// The largest hicolor PNG, for the GUI's preview pane.
    pub fn preview_png(&self) -> Option<&[u8]> {
        self.hicolor.last().map(|(_, b)| b.as_slice())
    }
}

/// Pad a non-square image into a square with transparency, centred.
///
/// The alternative is cropping, which silently eats the edges of someone's
/// artwork. Padding is visible, reversible and warned about.
fn pad_to_square(src: &RgbaImage) -> RgbaImage {
    let side = src.width().max(src.height());
    let mut out = RgbaImage::new(side, side);
    let dx = (side - src.width()) / 2;
    let dy = (side - src.height()) / 2;
    for (x, y, px) in src.enumerate_pixels() {
        out.put_pixel(x + dx, y + dy, *px);
    }
    out
}

fn encode_png(img: &RgbaImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| Error::Icon(format!("could not encode PNG: {e}")))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, alpha: u8) -> Vec<u8> {
        let mut img = RgbaImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, alpha]);
        }
        encode_png(&img).unwrap()
    }

    #[test]
    fn generates_all_three_formats() {
        let set = IconSet::from_png_bytes(&png(512, 512, 200)).unwrap();
        assert_eq!(&set.ico[0..4], &[0, 0, 1, 0], "ICO header");
        assert_eq!(&set.icns[0..4], b"icns", "ICNS magic");
        assert_eq!(set.hicolor.len(), HICOLOR_SIZES.len());
        for (size, bytes) in &set.hicolor {
            let img = image::load_from_memory(bytes).unwrap();
            assert_eq!(img.width(), *size);
            assert_eq!(img.height(), *size);
        }
    }

    #[test]
    fn a_good_source_produces_no_warnings() {
        let set = IconSet::from_png_bytes(&png(1024, 1024, 200)).unwrap();
        assert!(set.warnings.is_empty(), "unexpected: {:?}", set.warnings);
    }

    #[test]
    fn non_square_is_padded_not_cropped() {
        let set = IconSet::from_png_bytes(&png(600, 300, 200)).unwrap();
        assert!(set.warnings.contains(&IconWarning::NotSquare {
            width: 600,
            height: 300
        }));
        // Padded to 600×600, so nothing was upscaled beyond that.
        let big = image::load_from_memory(&set.hicolor.last().unwrap().1).unwrap();
        assert_eq!(big.width(), big.height());
    }

    #[test]
    fn small_and_opaque_sources_warn() {
        let set = IconSet::from_png_bytes(&png(128, 128, 255)).unwrap();
        assert!(set.warnings.contains(&IconWarning::Small { size: 128 }));
        assert!(set.warnings.contains(&IconWarning::Opaque));
        assert!(set
            .warnings
            .iter()
            .any(|w| matches!(w, IconWarning::Upscaled { .. })));
    }

    #[test]
    fn alpha_survives_the_pipeline() {
        let set = IconSet::from_png_bytes(&png(512, 512, 0)).unwrap();
        let (_, bytes) = set.hicolor.last().unwrap();
        let img = image::load_from_memory(bytes).unwrap().to_rgba8();
        assert!(
            img.pixels().all(|p| p.0[3] == 0),
            "transparency was flattened"
        );
    }

    #[test]
    fn garbage_input_is_an_error_not_a_panic() {
        assert!(IconSet::from_png_bytes(b"").is_err());
        assert!(IconSet::from_png_bytes(b"not a png").is_err());
    }
}
