use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::codecs::jpeg::JpegEncoder;
use image::{
    AnimationDecoder, DynamicImage, EncodableLayout, ExtendedColorType, Frame, GenericImageView,
    ImageEncoder, ImageFormat,
};
use imagequant::{Image as QImage, RGBA};
use std::io::{Cursor, Write};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

/// Compress image
/// - bytes: Image byte array (Uint8Array from frontend)
/// - quality: Compression quality (0-100, lower means worse quality)
/// - resize_percent: Size scaling factor (0-1, smaller means smaller size)
#[wasm_bindgen]
pub fn compress(bytes: &[u8], quality: u8, resize_percent: f32) -> Result<Vec<u8>, JsError> {
    // Load image
    let image = image::load_from_memory(bytes)?;
    // Resize image (not effective for GIF)
    let image = resize_image(image, resize_percent);
    // Get image format
    let format = image::guess_format(bytes)?;

    // Final encoded image data
    let mut output = Vec::new();

    match format {
        ImageFormat::Png => {
            // Quantify PNG image
            quantify_png_with_color_index(image, quality, &mut output)?;
        }
        ImageFormat::Jpeg | ImageFormat::WebP => {
            let quality = (quality as f32 * 0.75) as u8;
            let mut encoder = JpegEncoder::new_with_quality(&mut output, quality);
            encoder.write_image(
                image.as_bytes(),
                image.width(),
                image.height(),
                ExtendedColorType::from(image.color()),
            )?;
        }
        ImageFormat::Gif => {
            let decoder = GifDecoder::new(Cursor::new(bytes))?;
            let frames = decoder.into_frames();
            let frames = frames.collect_frames()?;

            let frames = frames
                .into_iter()
                .map(|frame| {
                    let image = frame.into_buffer();
                    let image = DynamicImage::from(image);
                    let image = resize_image(image, resize_percent);
                    let image = quantify_png_with_rgba(image, quality).unwrap();
                    Frame::new(image)
                })
                .collect::<Vec<_>>();

            let mut encoder = GifEncoder::new(&mut output);
            encoder.set_repeat(Repeat::Infinite)?;
            encoder.encode_frames(frames.into_iter())?;
        }
        _ => {
            return Err(JsError::new("Unsupported image format"));
        }
    }

    if output.len() > bytes.len() {
        return Ok(bytes.to_vec());
    }

    Ok(output)
}

fn resize_image(image: DynamicImage, resize_percent: f32) -> DynamicImage {
    if resize_percent == 1.0 {
        return image;
    }
    let (width, height) = (image.width(), image.height());
    let new_width = (width as f32 * resize_percent) as u32;
    let new_height = (height as f32 * resize_percent) as u32;
    image.resize(new_width, new_height, image::imageops::FilterType::Nearest)
}

/// Quantify PNG image using direct RGBA values
/// - image: Image to process
/// - quality: Compression quality (0-100, lower means worse quality)
fn quantify_png_with_rgba(image: DynamicImage, quality: u8) -> Result<image::RgbaImage, JsError> {
    let (width, height) = (image.width(), image.height());
    let (palette, pixels) = quantify_and_get_platte_and_indexes(image, quality)?;

    let mut buf = Vec::with_capacity(pixels.len());
    for index in pixels {
        // Get color from palette and convert to RGBA
        let rgba = palette[index as usize];
        buf.extend_from_slice(&[rgba.r, rgba.g, rgba.b, rgba.a]);
    }

    let rgba_image =
        image::RgbaImage::from_vec(width, height, buf).expect("Failed to create image");

    Ok(rgba_image)
}

/// Quantify PNG image using palette + index method
/// - image: Image to process
/// - quality: Compression quality (0-100, lower means worse quality)
/// - output: Output writer
fn quantify_png_with_color_index<W: Write>(
    image: DynamicImage,
    quality: u8,
    output: W,
) -> Result<(), JsError> {
    let (width, height) = (image.width(), image.height());

    let (palette, indexes) = quantify_and_get_platte_and_indexes(image, quality)?;

    // RGB palette
    let rgb_palette = palette
        .iter()
        .flat_map(|rgba| [rgba.r, rgba.g, rgba.b])
        .collect::<Vec<_>>();
    // Alpha channel values
    let alpha_values = palette.iter().map(|rgba| rgba.a).collect::<Vec<u8>>();

    let mut encoder = png::Encoder::new(output, width, height);
    encoder.set_palette(rgb_palette);
    encoder.set_trns(alpha_values);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Best);
    encoder.set_filter(png::FilterType::NoFilter);
    encoder.set_adaptive_filter(png::AdaptiveFilterType::NonAdaptive);

    let mut writer = encoder.write_header()?;
    writer.write_image_data(&indexes)?;

    Ok(())
}

/// Quantify PNG and get palette and indexes
/// - image: Image to process
/// - quality: Compression quality (0-100, lower means worse quality)
fn quantify_and_get_platte_and_indexes(
    image: DynamicImage,
    quality: u8,
) -> Result<(Vec<RGBA>, Vec<u8>), JsError> {
    let image = image.into_rgba8();
    let (width, height) = (image.width(), image.height());

    let mut quantizer = imagequant::new();
    quantizer.set_quality(0, quality)?;

    let rgba_data: Vec<RGBA> = image
        .as_bytes()
        .chunks_exact(4)
        .map(|chunk| RGBA {
            r: chunk[0],
            g: chunk[1],
            b: chunk[2],
            a: chunk[3],
        })
        .collect();

    // Quantified image
    let mut q_img = QImage::new(&quantizer, rgba_data, width as usize, height as usize, 0.)?;

    // Perform quantization
    let mut res = quantizer.quantize(&mut q_img)?;

    // Palette and indexes
    Ok(res.remapped(&mut q_img)?)
}
