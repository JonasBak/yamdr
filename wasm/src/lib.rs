use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn markdown_to_html(markdown: &str) -> String {
    let options = md::YamdrOptions {
        standalone: None,
        additional_head: None,
        additional_body: None,
        format: Some(md::Format::Html),
    };
    let (_meta, html) = md::render_markdown(&options, markdown);
    return html;
}
