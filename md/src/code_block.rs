use crate::{CustomBlock, CustomBlockHeader, CustomBlockReader, Format, Result};
use pulldown_cmark::{escape::escape_html, CodeBlockKind, Event, Tag};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

static HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "constant",
    "function.builtin",
    "function",
    "keyword",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "string",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
];

#[derive(Debug, Clone)]
pub struct CodeBlock {
    header: CustomBlockHeader,
    code: String,
}

pub struct CodeBlockReader {}

impl CodeBlockReader {
    pub fn initial_state() -> Self {
        CodeBlockReader {}
    }
}

impl CustomBlockReader for CodeBlockReader {
    fn can_read_block(&self, header: &CustomBlockHeader) -> bool {
        header.t == "Code"
    }

    fn read_block(
        &mut self,
        header: &CustomBlockHeader,
        input: &str,
    ) -> Result<Option<Box<dyn CustomBlock>>> {
        Ok(Some(Box::new(CodeBlock {
            header: header.clone(),
            code: input.into(),
        })))
    }
}

impl CustomBlock for CodeBlock {
    fn to_events(&self, format: Format) -> Vec<Event<'static>> {
        match format {
            Format::Html => {
                let filename = self.header.fields.get("filename").and_then(serde_yaml::Value::as_str);
                let language = self.header.fields.get("language").and_then(serde_yaml::Value::as_str);
                let numbered = self.header.fields.get("numbers").and_then(serde_yaml::Value::as_bool);
                let numbers_start_at = self.header.fields.get("numbers_start_at").and_then(serde_yaml::Value::as_u64);
                let open_tags = format!(
                    r#"<div><pre data-file="{}" class="codeblock language-{}"><code class="{}">"#,
                    filename.unwrap_or(""),
                    language.unwrap_or("none"),
                    if numbered
                        .unwrap_or(filename.is_some()) { "numbered" } else { "" },
                );
                let numbers_start_at = numbers_start_at.unwrap_or(1);
                let mut events = vec![Event::Html(open_tags.into())];
                let code = highlight(&self.code, language, true);
                for (i, line) in code.lines().enumerate() {
                    let line = format!(
                        r#"<span data-linenumber="{}|">{}</span>{}"#,
                        i as u64 + numbers_start_at,
                        line,
                        "\n"
                    );
                    events.push(Event::Html(line.into()));
                }
                events.push(Event::Html(r#"</code></pre></div>"#.into()));

                events
            }
            Format::Md => {
                let props: pulldown_cmark::CowStr =
                    serde_json::to_string(&CustomBlockHeader::empty("Code".into()))
                        .unwrap()
                        .into();
                let mut events = vec![Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(
                    props.clone(),
                )))];
                events.push(Event::Text(self.code.clone().into()));
                events.push(Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(props))));

                events
            }
        }
    }
}

pub fn highlight_config(lang: &str) -> Option<HighlightConfiguration> {
    match lang {
        "rust" => Some(
            HighlightConfiguration::new(
                tree_sitter_rust::language(),
                tree_sitter_rust::HIGHLIGHT_QUERY,
                "",
                "",
            )
            .unwrap(),
        ),
        "go" => Some(
            HighlightConfiguration::new(
                tree_sitter_go::language(),
                tree_sitter_go::HIGHLIGHT_QUERY,
                "",
                "",
            )
            .unwrap(),
        ),
        "javascript" => Some(
            HighlightConfiguration::new(
                tree_sitter_javascript::language(),
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                "",
                "",
            )
            .unwrap(),
        ),
        _ => None,
    }
}

pub fn highlight(code: &String, lang: Option<&str>, escape: bool) -> String {
    let mut highlighter = Highlighter::new();

    let mut config = if let Some(config) = lang.and_then(highlight_config) {
        config
    } else {
        if escape {
            let mut escaped = String::new();
            escape_html(&mut escaped, code).unwrap();
            return escaped;
        }
        return code.clone();
    };

    config.configure(HIGHLIGHT_NAMES);

    let highlights = highlighter
        .highlight(&config, code.as_bytes(), None, |_| None)
        .unwrap();

    let mut highlighted = String::new();

    let mut current_highlight: Option<usize> = None;

    for event in highlights {
        match event.unwrap() {
            HighlightEvent::Source { start, end } => {
                if let Some(highlight) = current_highlight {
                    for (i, line) in code[start..end].split('\n').enumerate() {
                        if i > 0 {
                            highlighted += "\n";
                        }
                        highlighted += r#"<span class="_"#;
                        highlighted += &HIGHLIGHT_NAMES[highlight].replace('.', "_");
                        highlighted += r#"">"#;
                        if escape {
                            escape_html(&mut highlighted, line).unwrap();
                        } else {
                            highlighted += line;
                        }
                        highlighted += r#"</span>"#;
                    }
                } else if escape {
                    escape_html(&mut highlighted, &code[start..end]).unwrap();
                } else {
                    highlighted += &code[start..end];
                }
            }
            HighlightEvent::HighlightStart(s) => {
                current_highlight = Some(s.0);
            }
            HighlightEvent::HighlightEnd => {
                current_highlight = None;
            }
        }
    }

    highlighted
}
