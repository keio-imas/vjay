use vjay_application::{clipboard::PatchFragment, ports::PortResult};
const HEADER: &str = "VJAY_PATCH_V1\n";
pub fn encode(fragment: &PatchFragment) -> PortResult<String> {
    let body = serde_json::to_string(fragment).map_err(|e| e.to_string())?;
    if body.len() + HEADER.len() > 16 * 1024 * 1024 {
        return Err("Clipboard patch exceeds 16 MiB".into());
    }
    Ok(format!("{HEADER}{body}"))
}
pub fn decode(text: &str) -> PortResult<PatchFragment> {
    if text.len() > 16 * 1024 * 1024 {
        return Err("Clipboard patch exceeds 16 MiB".into());
    }
    serde_json::from_str(
        text.strip_prefix(HEADER)
            .ok_or("Clipboard has no VJAY nodes")?,
    )
    .map_err(|e| format!("Invalid clipboard patch: {e}"))
}
