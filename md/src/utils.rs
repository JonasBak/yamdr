use pulldown_cmark::{escape::escape_html, CodeBlockKind, Event, Tag};

pub fn html_hide_with_title<'a>(title: String, events: impl IntoIterator<Item = Event<'a>>) -> Vec<Event<'a>> {
    let mut title_escaped = String::new();
    escape_html(&mut title_escaped, &title).unwrap();
    let mut e = vec![Event::Html(format!("<details><summary>{}</summary>", title_escaped).into())];
    e.extend(events.into_iter());
    e.push(Event::Html("</details>".into()));
    e
}
