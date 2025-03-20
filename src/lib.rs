use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{
    AnimationDecoder, DynamicImage, EncodableLayout, ExtendedColorType, Frame, GenericImageView,
    ImageEncoder, ImageFormat,
};
use imagequant::{Image as QImage, RGBA};
use png::chunk::PLTE;
use std::io::{Cursor, Read, Write};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

/// 压缩图片
/// - bytes: 图片字节数组，前端传Uint8Array
/// - quality: 压缩质量，0-100，越小质量越低
/// - resize_percent: 压缩尺寸，0-1，越小尺寸越低
#[wasm_bindgen]
pub fn compress(bytes: &[u8], quality: u8, resize_percent: f32) -> Result<Vec<u8>, JsError> {
    // 加载图像
    let image = image::load_from_memory(bytes)?;
    // 调整图片尺寸(对gif无效)
    let image = resize_image(image, resize_percent);
    // 获取图片格式
    let format = image::guess_format(bytes)?;

    // 最终编码后的图片数据
    let mut output = Vec::new();

    match format {
        ImageFormat::Png => {
            // 量化PNG图片
            quantify_png_with_color_index(image, quality, &mut output)?;
        }
        ImageFormat::Jpeg | ImageFormat::WebP => {
            let encoder = JpegEncoder::new_with_quality(&mut output, quality);
            encoder.write_image(
                image.as_bytes(),
                image.width(),
                image.height(),
                ExtendedColorType::Rgb8,
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

/// 量化PNG图片，使用RGBA直接颜色值
/// - image: 图片
/// - quality: 压缩质量，0-100，越小质量越低
fn quantify_png_with_rgba(image: DynamicImage, quality: u8) -> Result<image::RgbaImage, JsError> {
    let (palette, pixels) = quantify_and_get_platte_and_indexes(&image, quality)?;

    let mut buf = Vec::with_capacity(pixels.len());
    for index in pixels {
        // 从调色版中取出颜色，转为rgba
        let rgba = palette[index as usize];
        buf.extend_from_slice(&[rgba.r, rgba.g, rgba.b, rgba.a]);
    }

    let rgba_image = image::RgbaImage::from_vec(image.width(), image.height(), buf)
        .expect("Failed to create image");

    Ok(rgba_image)
}

/// 量化PNG图片，使用调色板+索引的方式
/// - image: 图片
/// - quality: 压缩质量，0-100，越小质量越低
/// - output: 写入的output
fn quantify_png_with_color_index<W: Write>(
    image: DynamicImage,
    quality: u8,
    output: W,
) -> Result<(), JsError> {
    let (palette, indexes) = quantify_and_get_platte_and_indexes(&image, quality)?;

    // RGB调色板
    let rgb_palette = palette
        .iter()
        .flat_map(|rgba| [rgba.r, rgba.g, rgba.b])
        .collect::<Vec<_>>();
    // 透明通道调色板
    let alpha_values = palette.iter().map(|rgba| rgba.a).collect::<Vec<u8>>();

    let mut encoder = png::Encoder::new(output, image.width(), image.height());
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

/// 量化PNG，并获取PNG图片的调色板和索引
/// - image: 图片
/// - quality: 压缩质量，0-100，越小质量越低
fn quantify_and_get_platte_and_indexes(
    image: &DynamicImage,
    quality: u8,
) -> Result<(Vec<RGBA>, Vec<u8>), JsError> {
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

    // 量化后的图片
    let mut q_img = QImage::new(&quantizer, rgba_data, width as usize, height as usize, 0.)?;

    // 执行量化
    let mut res = quantizer.quantize(&mut q_img)?;

    // 调色板和索引
    Ok(res.remapped(&mut q_img)?)
}
