mod code_block;
mod errors;
mod graph_block;
mod html;
mod md;
mod plotters_block;
mod script_block;
mod utils;

use code_block::CodeBlockReader;
pub use errors::*;
use graph_block::GraphBlockReader;
use plotters_block::PlottersBlockReader;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use script_block::ScriptBlockReader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trait that represents a reader/processor for one or more types
/// of custom blocks. Multiple readers may be able to process the same
/// type of custom block, in that case the first registered reader will
/// be chosen.
pub trait CustomBlockReader {
    /// Inspect a block header and return whether or not this reader should
    /// be the one to process this block. It it returns `true`, it indicated
    /// that you should be able to call `read_block` for that block.
    fn can_read_block(&self, _header: &CustomBlockHeader) -> bool {
        false
    }

    /// Process a block and return a CustomBlock.
    ///
    /// TODO: Option should be removed to always support "rerendering".
    /// The None case should be handled in to_events
    fn read_block(
        &mut self,
        _header: &CustomBlockHeader,
        _input: &str,
    ) -> Result<Option<Box<dyn CustomBlock>>> {
        unimplemented!()
    }

    /// Inspect inline code and return whether or not this reader should
    /// be the one to process the inline code. If `true`, it indicates that
    /// you should be able to call `read_inline` with the same input.
    fn can_read_inline(&self, _inline: &str) -> bool {
        false
    }
    /// Process inline code and return a CustomBlock.
    ///
    /// TODO: Option should be removed to always support "rerendering".
    fn read_inline(&mut self, _inline: &str) -> Result<Option<Box<dyn CustomBlock>>> {
        unimplemented!()
    }
}

/// Trait that represents a custom block that "extends" normal markdown
/// functionality.
///
/// A custom block should support "rerendering", that is that the output
/// of rendering it to markdown once, or rendering that output to markdown
/// again, should produce the same result. This is to ensure that markdown
/// to markdown rendering can be used as a formatter for yamdr documents,
/// without changing the semantics of the document.
pub trait CustomBlock {
    /// Render the block as a list of `pulldown_cmark::Event`s.
    ///
    /// Takes a `Format`, as implementations may differ when rendering to
    /// markdown or HTML. The `Vec` may be empty if nothing should be
    /// rendered, but this should only be for `Format::Html` (because of
    /// rerendering).
    fn to_events(&self, format: Format) -> Vec<Event<'static>>;

    /// This is a utility function that is used with
    /// `utils::custom_block_downcast` for easier testing.
    #[cfg(test)]
    fn as_any(&self) -> &dyn std::any::Any {
        unimplemented!()
    }
}

pub enum ExtendedEvent<'a> {
    Standard(Event<'a>),
    Custom(Box<dyn CustomBlock>),
    External(ExternalBlock),
    Separator(u16),
}

#[derive(Copy, Clone, PartialEq)]
pub enum Format {
    Html,
    Md,
}

impl Format {
    fn transform_extended_event<'a>(self, ee: &'a ExtendedEvent<'a>) -> Vec<Event<'a>> {
        match ee {
            ExtendedEvent::Standard(e) => vec![e.clone()],
            ExtendedEvent::Custom(c) => c.to_events(self),
            ExtendedEvent::Separator(_) => vec![],
            ExtendedEvent::External(_) => todo!(),
        }
    }
    fn render<'a>(self, events: impl Iterator<Item = Event<'a>>) -> String {
        match self {
            Format::Html => html::render(events),
            Format::Md => md::render(events),
        }
    }
}

pub static STYLE: &str = r#"
    html {
      font-family: sans;
      font-size: 16px;
      line-height: 1.5;
    }
    h1 {
      text-decoration: underline;
    }
    td {
      padding: 8px 12px;
    }
    code {
      background-color: #dcdcdc;
      padding: 0px 4px;
      border-radius: 4px;
    }
    div.script > pre {
      background-color: #dcdcdc;
      padding: 20px;
      border-radius: 4px;
      overflow-x: auto;
      font-size: 12px;
    }
    .script-output {
      font-weight: bold;
    }
    .content {
      max-width: 1000px;
      margin: auto;
    }
    .script {
    }
    .script-code {
    }
    .script-output {
    }
    .error {
        background-color: red;
        padding: 10px;
    }


    pre {
      overflow-x: auto;
      line-height: 1;
      font-size: 0.85em;
      padding: 5px 0px;
    }
    pre.codeblock {
      white-space: pre;
      padding: 10px 0px;
    }
    pre.codeblock.language-terminal {
      background-color: var(--codeblock-terminal-background);
      color: var(--codeblock-terminal-text);
    }
    pre.codeblock > code {
      display: block;
      margin-left: 2em;
    }
    pre.codeblock > code.numbered {
      margin-left: 0em;
    }
    pre.codeblock::before {
      content: attr(data-file);
      color: var(--codeblock-filename);
      font-family: monospace;
      display: block;
      margin-bottom: 2px;
      font-size: 1em;
    }
    code.numbered > span::before {
      content: attr(data-linenumber);
      text-align: right;
      color: var(--codeblock-linenumber);
      min-width: 3em;
      display: inline-block;
    }
"#;

#[derive(Clone)]
pub struct StandaloneOptions {}

#[derive(Clone)]
pub struct YamdrOptions {
    pub standalone: Option<StandaloneOptions>,
    pub additional_head: Option<String>,
    pub additional_body: Option<String>,
    pub format: Option<Format>,
}

pub struct Meta {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CustomBlockHeader {
    pub t: String,

    #[serde(flatten)]
    pub fields: HashMap<String, serde_yaml::Value>,
}

impl CustomBlockHeader {
    pub fn empty(t: String) -> Self {
        CustomBlockHeader {
            t,
            fields: HashMap::new(),
        }
    }
}

fn parse_markdown(markdown: &str) -> Vec<ExtendedEvent> {
    let md_options = Options::all();

    let mut readers: Vec<Box<dyn CustomBlockReader>> = vec![
        Box::new(ScriptBlockReader::initial_state()),
        Box::new(CodeBlockReader::initial_state()),
        Box::new(PlottersBlockReader::initial_state()),
        Box::new(GraphBlockReader::initial_state()),
    ];

    let mut current_custom_block: Option<CustomBlockHeader> = None;

    let mut level = 0;
    let mut element_i = 0;

    let parser = Parser::new_ext(markdown, md_options)
        .flat_map(|event| {
            match &event {
                Event::Start(_) => {
                    level += 1;
                    if level == 1 {
                        return vec![
                            Event::Start(Tag::FootnoteDefinition(
                                format!("yamdr:{}", element_i).into(),
                            )),
                            event,
                        ];
                    }
                }
                Event::End(_) => {
                    level -= 1;
                    if level == 0 {
                        element_i += 1;
                        return vec![
                            event,
                            Event::End(Tag::FootnoteDefinition(
                                format!("yamdr:{}", element_i - 1).into(),
                            )),
                        ];
                    }
                }
                _ => {}
            };
            vec![event]
        })
        .flat_map(|event| match &event {
            Event::Start(Tag::FootnoteDefinition(id)) if id.as_ref().starts_with("yamdr:") => {
                vec![ExtendedEvent::Separator(str::parse(&id[6..]).unwrap())]
            }
            Event::End(Tag::FootnoteDefinition(id)) if id.as_ref().starts_with("yamdr:") => {
                Vec::new()
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(prop))) => {
                match serde_yaml::from_str::<CustomBlockHeader>(prop) {
                    Ok(block) => {
                        current_custom_block = Some(block);
                        Vec::new()
                    }
                    Err(_) => {
                        vec![ExtendedEvent::Standard(event)]
                    }
                }
            }
            Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(_))) => {
                if current_custom_block.is_some() {
                    current_custom_block = None;
                    Vec::new()
                } else {
                    vec![ExtendedEvent::Standard(event)]
                }
            }
            Event::Text(text) if current_custom_block.is_some() => {
                let custom_block_header = current_custom_block.as_ref().unwrap();
                if custom_block_header.t == "External" {
                    return vec![ExtendedEvent::External(ExternalBlock {
                        body: text.to_string(),
                        head: custom_block_header.fields.clone(),
                    })];
                }
                match readers
                    .iter_mut()
                    .find(|reader| reader.can_read_block(custom_block_header))
                    .map(|reader| reader.read_block(custom_block_header, text))
                {
                    Some(Ok(Some(block))) => {
                        vec![ExtendedEvent::Custom(block)]
                    }
                    Some(Ok(None)) => Vec::new(),
                    Some(Err(_err)) => {
                        todo!("error reading block")
                    }
                    None => {
                        todo!("error custom block not implemented")
                    }
                }
            }
            Event::Code(code) => {
                match readers
                    .iter_mut()
                    .find(|reader| reader.can_read_inline(code))
                    .map(|reader| reader.read_inline(code))
                {
                    Some(Ok(Some(block))) => {
                        vec![ExtendedEvent::Custom(block)]
                    }
                    Some(Ok(None)) => Vec::new(),
                    Some(Err(_err)) => {
                        todo!("error reading inline")
                    }
                    None => {
                        vec![ExtendedEvent::Standard(event)]
                    }
                }
            }
            _ => vec![ExtendedEvent::Standard(event)],
        });

    parser.collect()
}

pub fn render_markdown(options: &YamdrOptions, markdown: &str) -> (Meta, String) {
    let format = options.format.unwrap_or(Format::Html);

    let parsed_markdown = parse_markdown(markdown);
    let parser = parsed_markdown
        .iter()
        .flat_map(|ee| format.transform_extended_event(ee));

    let mut output = format.render(parser);

    if format == Format::Html {
        if options.standalone.is_some() {
            output = format!(
                r#"
<!DOCTYPE html>
<html>
    <head>
        <style>
            {}
        </style>
        {}
    </head>
    <body>
        {}
        <div class="content">
            {}
        </div>
    </body>
</html>"#,
                STYLE,
                options.additional_head.as_deref().unwrap_or(""),
                options.additional_body.as_deref().unwrap_or(""),
                output
            );
        } else {
            output = format!(
                r#"
<style>
{}
</style>
{}
<div class="content">
{}
</div>"#,
                STYLE,
                options.additional_body.as_deref().unwrap_or(""),
                output
            );
        }
    }

    let meta = Meta {};

    (meta, output)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalBlock {
    pub body: String,
    pub head: HashMap<String, serde_yaml::Value>,
}

/// Representaion of a "top level" block of a markdown document. Contains
/// both the rendered html, and the "rerendered" markdown. If the block
/// is "external type", the block header and content can be accessed in
/// the `external` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownBlock {
    pub id: u16,
    pub html: String,
    pub markdown: String,
    pub external: Option<ExternalBlock>,
}

/// Representation of a markdown document, both as rendered html, and as
/// "rerendered" markdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownDocumentBlocks {
    pub css: String,
    pub blocks: Vec<MarkdownBlock>,
}

impl MarkdownDocumentBlocks {
    /// Rerender the contents of the markdown in each block. Useful when editing
    /// block by block, instead of entire documents.
    pub fn rerender(&mut self) {
        let markdown_document = self
            .blocks
            .iter()
            .map(|block| block.markdown.as_str())
            .collect::<Vec<&str>>()
            .join("\n");
        *self = render_blocks(&markdown_document);
    }
}

/// Parse a markdown document and return a MarkdownDocumentBlocks that contains
/// the blocks in the document, as well as css for the rendered html.
///
/// The blocks of the document are each "top level element", and are both rendered
/// as html, and "rerendered" as markdown. If a block is "external type", the `external`
/// field can be read to get the header and content of that block.
///
/// To build the complete html or markdown document, the `html` or `markdown` fields of
/// each block can be joined. The `id` might be useful if you need to find out which
/// block some html or markdown came from.
pub fn render_blocks(markdown: &str) -> MarkdownDocumentBlocks {
    let html = Format::Html;
    let md = Format::Md;
    let blocks = parse_markdown(markdown)
        .into_iter()
        .fold(Vec::new(), |mut acc, x| {
            match x {
                ExtendedEvent::Separator(id) => {
                    acc.push((id, Vec::new()));
                }
                _ => {
                    acc.last_mut().unwrap().1.push(x);
                }
            }
            acc
        })
        .into_iter()
        .map(|(id, events)| {
            if let &[ExtendedEvent::External(external)] = &events.as_slice() {
                return MarkdownBlock {
                    id,
                    html: "".into(),
                    markdown: "".into(), // TODO rerendering
                    external: Some(external.clone()),
                };
            }
            let html = html.render(
                events
                    .iter()
                    .flat_map(|ee| html.transform_extended_event(ee)),
            );
            let markdown = md.render(events.iter().flat_map(|ee| md.transform_extended_event(ee)));
            MarkdownBlock {
                id,
                html,
                markdown,
                external: None,
            }
        })
        .collect();
    MarkdownDocumentBlocks {
        css: STYLE.into(),
        blocks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_blocks() {
        let document = r#"
# Header

A paragraph.

```
Code block
```

New paragraph

- List
- List

```{t: External, test: 123}
External block
```
"#;
        let blocks = render_blocks(document);
        assert_eq!(blocks.blocks.len(), 6);
        assert_eq!(
            blocks.blocks[0].markdown,
            r#"# Header

"#
        );
        assert_eq!(
            blocks.blocks[1].markdown,
            r#"A paragraph.

"#
        );
        assert_eq!(
            blocks.blocks[2].markdown,
            r#"```
Code block
```

"#
        );
        assert_eq!(
            blocks.blocks[3].markdown,
            r#"New paragraph

"#
        );
        assert_eq!(
            blocks.blocks[4].markdown,
            r#"- List
- List

"#
        );
        assert_eq!(blocks.blocks[5].markdown, r#""#);
        let external = blocks.blocks[5].external.as_ref().unwrap();
        assert_eq!(
            external.body,
            r#"External block
"#
        );
        assert_eq!(external.head.get("test").unwrap().as_i64(), Some(123),);
    }

    #[test]
    fn test_rerender_markdown_document_blocks() {
        let document = r#"
# Header

A paragraph.

```
Code block
```

New paragraph

- List
- List
"#;
        let mut blocks = render_blocks(document);
        assert_eq!(blocks.blocks.len(), 5);
        blocks.blocks[1].markdown = r#"A changed paragraph.

New paragraph in same block"#
            .to_string();
        blocks.rerender();
        assert_eq!(blocks.blocks.len(), 6);
    }
}
