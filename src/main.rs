use image::{DynamicImage, ImageFormat};
use nu_plugin::{
    serve_plugin, EngineInterface, EvaluatedCall, MsgPackSerializer, Plugin, PluginCommand,
};
use nu_protocol::{Category, Example, PipelineData, ShellError, Signature, SyntaxShape, Value};
use std::io::Cursor;

pub struct ImagePlugin;

impl Plugin for ImagePlugin {
    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![
            Box::new(ImageResize),
            Box::new(ImageConvert),
        ]
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
    fn usage(&self) -> &str { "Resizes piped image binary data with optional aspect ratio maintenance." }
    fn category(&self) -> Category { Category::Filters }

    fn signature(&self) -> Signature {
        Signature::build("image resize")
            .named("width", SyntaxShape::Int, "target width pixel dimension", Some('w'))
            .named("height", SyntaxShape::Int, "target height pixel dimension", Some('h'))
            .switch("maintain-aspect", "preserves native proportions using targeted dimensions", Some('m'))
            .input_output_type(nu_protocol::Type::Binary, nu_protocol::Type::Binary)
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.head;
        let width: Option<u32> = call.get_flag("width")?.map(|i: i64| i as u32);
        let height: Option<u32> = call.get_flag("height")?.map(|i: i64| i as u32);
        let maintain_aspect = call.has_flag("maintain-aspect")?;

        if width.is_none() && height.is_none() {
            return Err(ShellError::MissingParameter { param_name: "--width or --height".into(), span });
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
        encode_pipeline(resized_img, format, span)
    }
}

// ================= COMMAND 2: image convert =================
pub struct ImageConvert;

impl PluginCommand for ImageConvert {
    type Plugin = ImagePlugin;

    fn name(&self) -> &str { "image convert" }
    fn usage(&self) -> &str { "Converts pipeline image payload format natively inside memory." }
    fn category(&self) -> Category { Category::Filters }

    fn signature(&self) -> Signature {
        Signature::build("image convert")
            .required("target_format", SyntaxShape::String, "Format to translate to (jpeg, png, webp, bmp)")
            .input_output_type(nu_protocol::Type::Binary, nu_protocol::Type::Binary)
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.head;
        let target_str: String = call.req(0)?;

        let target_format = match target_str.to_lowercase().as_str() {
            "jpg" | "jpeg" => ImageFormat::Jpeg,
            "png" => ImageFormat::Png,
            "webp" => ImageFormat::WebP,
            "bmp" => ImageFormat::Bmp,
            _ => return Err(ShellError::GenericError {
                error: "Unsupported target format".into(),
                msg: format!("'{}' is not supported.", target_str),
                span: Some(span),
                help: Some("Use: jpeg, png, webp, or bmp".into()),
                inner: vec![],
            }),
        };

        // Extract original data, transcode parameters inside memory block, send downstream
        let (img, _) = decode_pipeline(input, span)?;
        encode_pipeline(img, target_format, span)
    }
}

fn main() {
    serve_plugin(&ImagePlugin, MsgPackSerializer);
}
