use image::{DynamicImage, ImageFormat};
use nu_plugin::{
    serve_plugin, EngineInterface, EvaluatedCall, MsgPackSerializer, Plugin, PluginCommand,
};
use nu_protocol::{Category, LabeledError, PipelineData, ShellError, Signature, SyntaxShape, Value};
use std::io::Cursor;

pub struct ImagePlugin;

impl Plugin for ImagePlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![
            Box::new(ImageResize),
            Box::new(ImageConvert),
        ]
    }
}

// Helper: Resolves a user-supplied format string to an ImageFormat
fn parse_image_format(s: &str) -> Option<ImageFormat> {
    match s.to_lowercase().as_str() {
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "png" => Some(ImageFormat::Png),
        "webp" => Some(ImageFormat::WebP),
        "bmp" => Some(ImageFormat::Bmp),
        _ => None,
    }
}

// Helper: Decodes raw pipe bytes into a manipulative DynamicImage object
fn decode_pipeline(input: PipelineData, span: nu_protocol::Span) -> Result<(DynamicImage, ImageFormat), ShellError> {
    let bytes = input.into_value(span)?.coerce_into_binary()?;
    let format = image::guess_format(&bytes).map_err(|e| {
        ShellError::GenericError {
            error: "Failed to recognize image format".into(),
            msg: e.to_string(),
            span: Some(span),
            help: Some("Ensure you are piping valid png, jpeg, or webp binary data.".into()),
            inner: vec![],
        }
    })?;
    
    let img = image::load_from_memory_with_format(&bytes, format).map_err(|e| {
        ShellError::GenericError {
            error: "Image Decode Error".into(),
            msg: e.to_string(),
            span: Some(span),
            help: None,
            inner: vec![],
        }
    })?;
    Ok((img, format))
}

// Helper: Encodes the processed image back into binary format for the next pipe segment
fn encode_pipeline(img: DynamicImage, format: ImageFormat, span: nu_protocol::Span) -> Result<PipelineData, ShellError> {
    let mut out_bytes = Vec::new();
    img.write_to(&mut Cursor::new(&mut out_bytes), format).map_err(|e| {
        ShellError::GenericError {
            error: "Image Encode Error".into(),
            msg: e.to_string(),
            span: Some(span),
            help: None,
            inner: vec![],
        }
    })?;
    Ok(PipelineData::Value(Value::binary(out_bytes, span), None))
}

// ================= COMMAND 1: image resize =================
pub struct ImageResize;

impl PluginCommand for ImageResize {
    type Plugin = ImagePlugin;

    fn name(&self) -> &str { "image resize" }
    fn description(&self) -> &str { "Resizes piped image binary data with optional aspect ratio maintenance." }

    fn signature(&self) -> Signature {
        Signature::build("image resize")
            .named("width", SyntaxShape::Int, "target width pixel dimension", Some('w'))
            .named("height", SyntaxShape::Int, "target height pixel dimension", Some('h'))
            .switch("maintain-aspect", "preserves native proportions using targeted dimensions", Some('m'))
            .input_output_type(nu_protocol::Type::Binary, nu_protocol::Type::Binary)
            .category(Category::Filters)
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let width: Option<u32> = call.get_flag("width")?.map(|i: i64| i as u32);
        let height: Option<u32> = call.get_flag("height")?.map(|i: i64| i as u32);
        let maintain_aspect = call.has_flag("maintain-aspect")?;

        if width.is_none() && height.is_none() {
            return Err(ShellError::MissingParameter { param_name: "--width or --height".into(), span }.into());
        }

        // 1. Grab image from pipeline
        let (img, format) = decode_pipeline(input, span)?;

        // 2. Perform math & Resize
        let resized_img = if maintain_aspect {
            let w = width.unwrap_or(img.width());
            let h = height.unwrap_or(img.height());
            img.resize(w, h, image::imageops::FilterType::Lanczos3)
        } else {
            let w = width.unwrap_or_else(|| img.width());
            let h = height.unwrap_or_else(|| img.height());
            img.resize_exact(w, h, image::imageops::FilterType::Lanczos3)
        };

        // 3. Return binary downstream
        Ok(encode_pipeline(resized_img, format, span)?)
    }
}

// ================= COMMAND 2: image convert =================
pub struct ImageConvert;

impl PluginCommand for ImageConvert {
    type Plugin = ImagePlugin;

    fn name(&self) -> &str { "image convert" }
    fn description(&self) -> &str { "Converts pipeline image payload format natively inside memory." }

    fn signature(&self) -> Signature {
        Signature::build("image convert")
            .required("target_format", SyntaxShape::String, "Format to translate to (jpeg, png, webp, bmp)")
            .input_output_type(nu_protocol::Type::Binary, nu_protocol::Type::Binary)
            .category(Category::Filters)
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let target_str: String = call.req(0)?;

        let target_format = match parse_image_format(&target_str) {
            Some(fmt) => fmt,
            None => return Err(ShellError::GenericError {
                error: "Unsupported target format".into(),
                msg: format!("'{}' is not supported.", target_str),
                span: Some(span),
                help: Some("Use: jpeg, png, webp, or bmp".into()),
                inner: vec![],
            }.into()),
        };

        // Extract original data, transcode parameters inside memory block, send downstream
        let (img, _) = decode_pipeline(input, span)?;
        Ok(encode_pipeline(img, target_format, span)?)
    }
}

fn main() {
    serve_plugin(&ImagePlugin, MsgPackSerializer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use nu_protocol::Span;

    // ---- helpers ----

    fn make_png_bytes(width: u32, height: u32) -> Vec<u8> {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
        let dyn_img = DynamicImage::ImageRgb8(img);
        let mut buf = Vec::new();
        dyn_img
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .expect("encode test PNG");
        buf
    }

    fn make_jpeg_bytes(width: u32, height: u32) -> Vec<u8> {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
        let dyn_img = DynamicImage::ImageRgb8(img);
        let mut buf = Vec::new();
        dyn_img
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)
            .expect("encode test JPEG");
        buf
    }

    fn pipeline_from_bytes(bytes: Vec<u8>, span: Span) -> PipelineData {
        PipelineData::Value(Value::binary(bytes, span), None)
    }

    // ---- decode_pipeline / encode_pipeline ----

    #[test]
    fn test_decode_encode_roundtrip_png() {
        let span = Span::test_data();
        let original = make_png_bytes(10, 10);
        let pipeline = pipeline_from_bytes(original.clone(), span);
        let (img, fmt) = decode_pipeline(pipeline, span).expect("decode");
        assert_eq!(fmt, ImageFormat::Png);
        assert_eq!(img.width(), 10);
        assert_eq!(img.height(), 10);
        encode_pipeline(img, fmt, span).expect("encode");
    }

    #[test]
    fn test_decode_encode_roundtrip_jpeg() {
        let span = Span::test_data();
        let original = make_jpeg_bytes(8, 8);
        let pipeline = pipeline_from_bytes(original, span);
        let (img, fmt) = decode_pipeline(pipeline, span).expect("decode");
        assert_eq!(fmt, ImageFormat::Jpeg);
        assert_eq!(img.width(), 8);
        assert_eq!(img.height(), 8);
    }

    #[test]
    fn test_decode_invalid_bytes_returns_error() {
        let span = Span::test_data();
        let pipeline = pipeline_from_bytes(b"not an image".to_vec(), span);
        assert!(decode_pipeline(pipeline, span).is_err());
    }

    // ---- image resize ----

    #[test]
    fn test_resize_exact_width_and_height() {
        let span = Span::test_data();
        let png = make_png_bytes(100, 100);
        let pipeline = pipeline_from_bytes(png, span);
        let (img, fmt) = decode_pipeline(pipeline, span).expect("decode");

        let resized = img.resize_exact(50, 30, image::imageops::FilterType::Lanczos3);
        assert_eq!(resized.width(), 50);
        assert_eq!(resized.height(), 30);
        assert_eq!(fmt, ImageFormat::Png);
    }

    #[test]
    fn test_resize_maintain_aspect_ratio() {
        let span = Span::test_data();
        let png = make_png_bytes(200, 100);
        let pipeline = pipeline_from_bytes(png, span);
        let (img, _) = decode_pipeline(pipeline, span).expect("decode");

        // Ask for at most 50×50; aspect-aware resize keeps both dims ≤ 50
        let resized = img.resize(50, 50, image::imageops::FilterType::Lanczos3);
        assert!(resized.width() <= 50, "width {} should be ≤ 50", resized.width());
        assert!(resized.height() <= 50, "height {} should be ≤ 50", resized.height());
    }

    #[test]
    fn test_resize_only_width_keeps_height() {
        let span = Span::test_data();
        let png = make_png_bytes(100, 60);
        let pipeline = pipeline_from_bytes(png, span);
        let (img, _) = decode_pipeline(pipeline, span).expect("decode");

        // When only width is provided (exact mode), height falls back to original
        let w = 40u32;
        let h = img.height(); // 60
        let resized = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
        assert_eq!(resized.width(), 40);
        assert_eq!(resized.height(), 60);
    }

    // ---- image convert ----

    #[test]
    fn test_convert_png_to_jpeg() {
        let span = Span::test_data();
        let png = make_png_bytes(8, 8);
        let pipeline = pipeline_from_bytes(png, span);
        let (img, _) = decode_pipeline(pipeline, span).expect("decode");

        let result = encode_pipeline(img, ImageFormat::Jpeg, span).expect("encode to jpeg");
        let out_bytes = result
            .into_value(span)
            .expect("value")
            .coerce_into_binary()
            .expect("binary");

        // JPEG magic bytes: FF D8 FF
        assert_eq!(&out_bytes[..3], &[0xFF, 0xD8, 0xFF]);
    }

    #[test]
    fn test_convert_png_to_bmp() {
        let span = Span::test_data();
        let png = make_png_bytes(8, 8);
        let pipeline = pipeline_from_bytes(png, span);
        let (img, _) = decode_pipeline(pipeline, span).expect("decode");

        let result = encode_pipeline(img, ImageFormat::Bmp, span).expect("encode to bmp");
        let out_bytes = result
            .into_value(span)
            .expect("value")
            .coerce_into_binary()
            .expect("binary");

        // BMP magic bytes: BM
        assert_eq!(&out_bytes[..2], b"BM");
    }

    #[test]
    fn test_convert_jpeg_to_png() {
        let span = Span::test_data();
        let jpeg = make_jpeg_bytes(8, 8);
        let pipeline = pipeline_from_bytes(jpeg, span);
        let (img, _) = decode_pipeline(pipeline, span).expect("decode");

        let result = encode_pipeline(img, ImageFormat::Png, span).expect("encode to png");
        let out_bytes = result
            .into_value(span)
            .expect("value")
            .coerce_into_binary()
            .expect("binary");

        // PNG magic bytes: 89 50 4E 47
        assert_eq!(&out_bytes[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_convert_unsupported_format_error() {
        assert!(parse_image_format("tiff").is_none(), "unsupported format should return None");
        assert!(parse_image_format("svg").is_none());
        assert!(parse_image_format("").is_none());
    }

    #[test]
    fn test_convert_format_aliases() {
        assert_eq!(parse_image_format("jpg"), Some(ImageFormat::Jpeg));
        assert_eq!(parse_image_format("jpeg"), Some(ImageFormat::Jpeg));
        assert_eq!(parse_image_format("JPG"), Some(ImageFormat::Jpeg));
        assert_eq!(parse_image_format("PNG"), Some(ImageFormat::Png));
        assert_eq!(parse_image_format("webp"), Some(ImageFormat::WebP));
        assert_eq!(parse_image_format("bmp"), Some(ImageFormat::Bmp));
    }
}
