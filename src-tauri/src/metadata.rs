//! Copying EXIF and ICC data from a source image onto the re-encoded output.
//!
//! Re-encoding always produces a bare file, so preserving metadata means explicitly moving
//! it across. The subtle part is orientation: the decoder already baked the EXIF rotation
//! into the pixels, so copying the tag unchanged would make viewers rotate the image a
//! second time. The tag is reset to "no transform" on the way out.

use img_parts::{Bytes, DynImage, ImageEXIF, ImageICC};

const ORIENTATION_TAG: u16 = 0x0112;
const NO_TRANSFORM: u16 = 1;

/// EXIF and ICC blobs lifted from a source file, ready to attach to an output file.
#[derive(Default, Clone)]
pub struct Metadata {
    exif: Option<Bytes>,
    icc: Option<Bytes>,
}

impl Metadata {
    pub fn is_empty(&self) -> bool {
        self.exif.is_none() && self.icc.is_none()
    }
}

/// Reads metadata out of an encoded source file. Unknown containers simply yield nothing.
pub fn read(source_bytes: &[u8]) -> Metadata {
    let parsed = DynImage::from_bytes(Bytes::copy_from_slice(source_bytes));

    let Ok(Some(image)) = parsed else {
        return Metadata::default();
    };

    Metadata {
        exif: image.exif().map(|exif| match neutralize_orientation(&exif) {
            Some(patched) => Bytes::from(patched),
            None => exif,
        }),
        icc: image.icc_profile(),
    }
}

/// Removes EXIF and ICC data from an encoded file without touching its pixels. Used by the
/// passthrough path, where re-encoding just to drop metadata would be wasteful and lossy.
pub fn strip(file_bytes: Vec<u8>) -> Vec<u8> {
    let parsed = DynImage::from_bytes(Bytes::from(file_bytes.clone()));

    let Ok(Some(mut image)) = parsed else {
        return file_bytes;
    };

    if image.exif().is_none() && image.icc_profile().is_none() {
        return file_bytes;
    }

    image.set_exif(None);
    image.set_icc_profile(None);

    let mut written = Vec::with_capacity(file_bytes.len());
    match image.encoder().write_to(&mut written) {
        Ok(_) => written,
        Err(_) => file_bytes,
    }
}

/// Attaches metadata to an encoded output file, returning the new bytes. Falls back to the
/// untouched output if the container cannot carry metadata.
pub fn apply(output_bytes: Vec<u8>, metadata: &Metadata) -> Vec<u8> {
    if metadata.is_empty() {
        return output_bytes;
    }

    let original_length = output_bytes.len();
    let parsed = DynImage::from_bytes(Bytes::from(output_bytes.clone()));

    let Ok(Some(mut image)) = parsed else {
        return output_bytes;
    };

    image.set_exif(metadata.exif.clone());
    image.set_icc_profile(metadata.icc.clone());

    let mut written = Vec::with_capacity(original_length);
    match image.encoder().write_to(&mut written) {
        Ok(_) => written,
        Err(_) => output_bytes,
    }
}

/// Rewrites the EXIF orientation tag to "no transform" in place.
///
/// The blob starts at the TIFF header: byte order, magic 42, then the offset of IFD0, which
/// holds 12-byte entries of tag / type / count / value.
fn neutralize_orientation(exif: &[u8]) -> Option<Vec<u8>> {
    let mut data = exif.to_vec();
    if data.len() < 8 {
        return None;
    }

    let little_endian = match &data[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };

    let read_u16 = |data: &[u8], at: usize| -> u16 {
        let pair = [data[at], data[at + 1]];
        if little_endian {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        }
    };

    if read_u16(&data, 2) != 42 {
        return None;
    }

    let quad = [data[4], data[5], data[6], data[7]];
    let ifd0 = if little_endian {
        u32::from_le_bytes(quad)
    } else {
        u32::from_be_bytes(quad)
    } as usize;

    if ifd0.checked_add(2)? > data.len() {
        return None;
    }

    let entries = read_u16(&data, ifd0) as usize;

    for index in 0..entries {
        let entry = ifd0 + 2 + index * 12;
        if entry + 12 > data.len() {
            return None;
        }

        if read_u16(&data, entry) == ORIENTATION_TAG {
            // A SHORT value lives in the first two bytes of the four byte value field.
            let value_at = entry + 8;
            let bytes = if little_endian {
                NO_TRANSFORM.to_le_bytes()
            } else {
                NO_TRANSFORM.to_be_bytes()
            };
            data[value_at] = bytes[0];
            data[value_at + 1] = bytes[1];
            data[value_at + 2] = 0;
            data[value_at + 3] = 0;
            return Some(data);
        }
    }

    // No orientation tag at all; the rest of the metadata is still worth keeping.
    Some(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal little-endian EXIF blob with a single orientation entry.
    fn exif_with_orientation(value: u16) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"II"); // little endian
        data.extend_from_slice(&42u16.to_le_bytes());
        data.extend_from_slice(&8u32.to_le_bytes()); // IFD0 starts right after the header
        data.extend_from_slice(&1u16.to_le_bytes()); // one entry
        data.extend_from_slice(&ORIENTATION_TAG.to_le_bytes());
        data.extend_from_slice(&3u16.to_le_bytes()); // SHORT
        data.extend_from_slice(&1u32.to_le_bytes()); // count
        data.extend_from_slice(&value.to_le_bytes());
        data.extend_from_slice(&[0, 0]); // padding of the value field
        data.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
        data
    }

    fn orientation_of(exif: &[u8]) -> u16 {
        // Entry value sits at: header (8) + entry count (2) + tag/type/count (8).
        u16::from_le_bytes([exif[18], exif[19]])
    }

    #[test]
    fn rotation_tag_is_reset_so_viewers_do_not_rotate_twice() {
        let exif = exif_with_orientation(6); // "rotate 90 clockwise"

        let patched = neutralize_orientation(&exif).expect("blob should parse");

        assert_eq!(orientation_of(&patched), NO_TRANSFORM);
    }

    #[test]
    fn big_endian_blobs_are_handled() {
        let mut data = Vec::new();
        data.extend_from_slice(b"MM");
        data.extend_from_slice(&42u16.to_be_bytes());
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&ORIENTATION_TAG.to_be_bytes());
        data.extend_from_slice(&3u16.to_be_bytes());
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&8u16.to_be_bytes()); // "rotate 270"
        data.extend_from_slice(&[0, 0]);
        data.extend_from_slice(&0u32.to_be_bytes());

        let patched = neutralize_orientation(&data).expect("blob should parse");

        assert_eq!(u16::from_be_bytes([patched[18], patched[19]]), NO_TRANSFORM);
    }

    #[test]
    fn other_entries_survive_untouched() {
        let exif = exif_with_orientation(3);
        let patched = neutralize_orientation(&exif).unwrap();

        assert_eq!(patched.len(), exif.len());
        assert_eq!(&patched[0..8], &exif[0..8], "TIFF header must not move");
    }

    #[test]
    fn garbage_is_rejected_rather_than_corrupted() {
        assert!(neutralize_orientation(b"not exif at all").is_none());
        assert!(neutralize_orientation(&[]).is_none());
    }
}
