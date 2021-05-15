use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/resources"]
#[prefix = "template/"]
/// Abstraction for the `/resources/template` folder path.
pub struct Template;
