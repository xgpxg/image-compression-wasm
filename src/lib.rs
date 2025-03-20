use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{
    AnimationDecoder, DynamicImage, EncodableLayout, ExtendedColorType, Frame, ImageEncoder,
    ImageFormat,
};
use imagequant::{Image as QImage, RGBA};
use std::io::Cursor;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

/// 压缩图片
/// - bytes: 图片字节数组，前端传Uint8Array
/// - quality: 压缩质量，0-100，越小质量越低
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
            let rgba_image = quantify_png(image, quality)?;

            // 重新编码
            let encoder = PngEncoder::new_with_quality(
                &mut output,
                CompressionType::Best,
                FilterType::NoFilter,
            );

            encoder.write_image(
                rgba_image.as_bytes(),
                rgba_image.width(),
                rgba_image.height(),
                ExtendedColorType::Rgba8,
            )?;
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
                    let image = quantify_png(image, quality).unwrap();
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
pub fn quantify_png(image: DynamicImage, quality: u8) -> Result<image::RgbaImage, JsError> {
    let original_image = image.into_rgba8(); //.expect("Failed to convert image to RGBA");
    let (width, height) = (original_image.width(), original_image.height());

    let mut quantizer = imagequant::new();
    quantizer.set_quality(0, quality)?;

    let rgba_data: Vec<RGBA> = original_image
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

    // 重新映射像素值
    // palette为颜色表，pixels是在palette中的索引
    let (palette, pixels) = res.remapped(&mut q_img)?;

    let mut buf = Vec::with_capacity(pixels.len());
    for index in pixels {
        // 从颜色表中取出颜色
        let rgba = palette[index as usize];
        buf.extend_from_slice(&[rgba.r, rgba.g, rgba.b, rgba.a]);
    }

    let rgba_image =
        image::RgbaImage::from_vec(width, height, buf).expect("Failed to create image");

    Ok(rgba_image)
}
