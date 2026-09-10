use std::fs;
use std::io::Read;

use serde::Deserialize;
use tauri::Manager;

// A realistic desktop-browser UA is required: fonts.googleapis.com serves
// old-style ttf/eot font formats to unrecognized/bare user agents and only
// hands out woff2 to modern browsers.
const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

#[derive(Deserialize)]
struct FontFamilyMeta {
    family: String,
    category: String,
}

#[derive(Deserialize)]
struct FontsMetadata {
    #[serde(rename = "familyMetadataList")]
    family_metadata_list: Vec<FontFamilyMeta>,
}

#[tauri::command]
pub fn list_google_monospace_fonts() -> Result<Vec<String>, String> {
    let body = ureq::get("https://fonts.google.com/metadata/fonts")
        .set("User-Agent", BROWSER_USER_AGENT)
        .call()
        .map_err(|err| err.to_string())?
        .into_string()
        .map_err(|err| err.to_string())?;

    // Google prefixes this JSON endpoint with `)]}'` to prevent naive JSON
    // hijacking via a plain <script> tag; strip it before parsing.
    let json_str = body.strip_prefix(")]}'").unwrap_or(&body).trim_start();
    let data: FontsMetadata = serde_json::from_str(json_str).map_err(|err| err.to_string())?;

    let mut families: Vec<String> = data
        .family_metadata_list
        .into_iter()
        .filter(|f| f.category == "Monospace")
        .map(|f| f.family)
        .collect();
    families.sort();
    Ok(families)
}

fn sanitize_family_for_filename(family: &str) -> String {
    family
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == ' ')
        .collect::<String>()
        .replace(' ', "_")
}

#[tauri::command]
pub fn install_google_font(app: tauri::AppHandle, family: String) -> Result<String, String> {
    let query = family.replace(' ', "+");
    let css_url = format!("https://fonts.googleapis.com/css2?family={query}&display=swap");
    let css = ureq::get(&css_url)
        .set("User-Agent", BROWSER_USER_AGENT)
        .call()
        .map_err(|err| err.to_string())?
        .into_string()
        .map_err(|err| err.to_string())?;

    // Google's css2 endpoint returns one @font-face block per unicode-range
    // subset (cyrillic, greek, vietnamese, latin-ext, latin, ...), in that
    // order — the "latin" block (basic ASCII + Latin-1, U+0000-00FF) is what
    // actually renders normal terminal text and is NOT necessarily first.
    let font_url = css
        .split("@font-face")
        .find(|block| block.contains("U+0000-00FF"))
        .or_else(|| css.split("@font-face").last())
        .and_then(|block| block.split("url(").nth(1))
        .and_then(|part| part.split(')').next())
        .ok_or_else(|| format!("No se encontro el archivo de fuente para \"{family}\""))?
        .to_string();

    let resp = ureq::get(&font_url)
        .set("User-Agent", BROWSER_USER_AGENT)
        .call()
        .map_err(|err| err.to_string())?;
    let mut bytes = Vec::new();
    resp.into_reader()
        .read_to_end(&mut bytes)
        .map_err(|err| err.to_string())?;

    let dir = app
        .path()
        .app_config_dir()
        .map_err(|err| err.to_string())?
        .join("fonts");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;

    let filename = format!("{}.woff2", sanitize_family_for_filename(&family));
    let path = dir.join(filename);
    fs::write(&path, &bytes).map_err(|err| err.to_string())?;

    Ok(path.to_string_lossy().into_owned())
}
