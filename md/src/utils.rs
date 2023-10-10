use pulldown_cmark::{escape::escape_html, CodeBlockKind, Event, Tag};
use rhai::plugin::Dynamic;

pub fn html_hide_with_title<'a>(
    title: String,
    events: impl IntoIterator<Item = Event<'a>>,
) -> Vec<Event<'a>> {
    let mut title_escaped = String::new();
    escape_html(&mut title_escaped, &title).unwrap();
    let mut e = vec![Event::Html(
        format!("<details><summary>{}</summary>", title_escaped).into(),
    )];
    e.extend(events.into_iter());
    e.push(Event::Html("</details>".into()));
    e
}

pub fn dynamic_as_f64(v: &Dynamic) -> Option<f64> {
    v.as_float()
        .ok()
        .or_else(|| v.as_int().map(|v| v as f64).ok())
}
