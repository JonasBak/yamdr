use pulldown_cmark::{escape::escape_html, html, CodeBlockKind, Event, Options, Parser, Tag};

pub fn render<'a>(events: impl Iterator<Item = Event<'a>>) -> String {
    let mut html_output = String::new();
    html::push_html(&mut html_output, events);
    html_output
}
