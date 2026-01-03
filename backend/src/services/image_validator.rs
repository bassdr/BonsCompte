use crate::error::{AppError, AppResult};

// Maximum file size: 5MB
const MAX_IMAGE_SIZE: usize = 5 * 1024 * 1024;

// Magic bytes for common image formats
const JPEG_START: [u8; 3] = [0xFF, 0xD8, 0xFF];
const PNG_START: [u8; 4] = [0x89, 0x50, 0x4E, 0x47];
const GIF87_START: &[u8] = b"GIF87a";
const GIF89_START: &[u8] = b"GIF89a";
const WEBP_START: &[u8] = b"RIFF";
const WEBP_CHUNK: &[u8] = b"WEBP";

/// Validates a base64-encoded image
/// - Checks file size
/// - Validates it's an actual image by checking magic bytes
pub fn validate_image_base64(base64_data: &str) -> AppResult<()> {
    // Decode base64
    let image_data = decode_base64(base64_data)
        .map_err(|_| AppError::BadRequest("Invalid base64 image data".to_string()))?;

    // Check size
    if image_data.len() > MAX_IMAGE_SIZE {
        return Err(AppError::BadRequest(format!(
            "Image exceeds maximum size of 5MB (actual: {:.2}MB)",
            image_data.len() as f64 / (1024.0 * 1024.0)
        )));
    }

    if image_data.is_empty() {
        return Err(AppError::BadRequest("Image data is empty".to_string()));
    }

    // Validate image format by magic bytes
    if !is_valid_image_format(&image_data) {
        return Err(AppError::BadRequest(
            "Invalid image format. Only JPEG, PNG, GIF, and WebP are supported".to_string(),
        ));
    }

    Ok(())
}

/// Decode base64 string to bytes
fn decode_base64(data: &str) -> Result<Vec<u8>, String> {
    // Remove data URI prefix if present (e.g., "data:image/png;base64,")
    let data = if let Some(pos) = data.find(",") {
        &data[pos + 1..]
    } else {
        data
    };

    // Standard base64 decoding using std library
    let mut result = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;

    for c in data.chars() {
        if c.is_whitespace() {
            continue;
        }

        let val = match c {
            'A'..='Z' => (c as u32) - ('A' as u32),
            'a'..='z' => (c as u32) - ('a' as u32) + 26,
            '0'..='9' => (c as u32) - ('0' as u32) + 52,
            '+' => 62,
            '/' => 63,
            '=' => break,
            _ => return Err("Invalid base64 character".to_string()),
        };

        buf = (buf << 6) | val;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            result.push(((buf >> bits) & 0xFF) as u8);
        }
    }

    Ok(result)
}

/// Check if the image data has valid magic bytes for supported formats
fn is_valid_image_format(data: &[u8]) -> bool {
    if data.len() < 3 {
        return false;
    }

    // Check JPEG (starts with FF D8 FF)
    if data.len() >= 3 && data[0..3] == JPEG_START {
        return true;
    }

    // Check PNG (starts with 89 50 4E 47)
    if data.len() >= 4 && data[0..4] == PNG_START {
        return true;
    }

    // Check GIF (starts with "GIF87a" or "GIF89a")
    if data.len() >= 6 && (&data[0..6] == GIF87_START || &data[0..6] == GIF89_START) {
        return true;
    }

    // Check WebP (starts with "RIFF" and contains "WEBP")
    if data.len() >= 12 && &data[0..4] == WEBP_START && &data[8..12] == WEBP_CHUNK {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_png() {
        // Minimal valid PNG header
        let png_header = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        assert!(validate_image_base64(png_header).is_ok());
    }

    #[test]
    fn test_reject_invalid_base64() {
        let invalid = "not-valid-base64!!!";
        assert!(validate_image_base64(invalid).is_err());
    }

    #[test]
    fn test_reject_non_image_data() {
        // Valid base64, but not an image
        let text = "VGhpcyBpcyBub3QgYW4gaW1hZ2U="; // "This is not an image"
        let err = validate_image_base64(text);
        assert!(err.is_err());
        assert!(err
            .unwrap_err()
            .to_string()
            .contains("Invalid image format"));
    }

    #[test]
    fn test_decode_base64() {
        let hello = "SGVsbG8sIFdvcmxkIQ=="; // "Hello, World!"
        let decoded = decode_base64(hello).unwrap();
        assert_eq!(decoded, b"Hello, World!");
    }

    #[test]
    fn test_decode_base64_with_data_uri() {
        let data_uri = "data:image/png;base64,SGVsbG8="; // "Hello"
        let decoded = decode_base64(data_uri).unwrap();
        assert_eq!(decoded, b"Hello");
    }
}
