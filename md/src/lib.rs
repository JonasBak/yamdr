mod graph_block;
mod html;
mod md;
mod script_block;

use graph_block::{GraphBlock, GraphState};
use miniserde::{json, Deserialize};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use script_block::{ScriptBlock, ScriptState};

trait CustomBlockState: Sized {
    type Block: CustomBlock;

    fn initial_state() -> Self;

    fn read_block(
        &mut self,
        header: &CustomBlockHeader,
        input: &str,
    ) -> Result<Option<Self::Block>, String>;
}

trait CustomBlock {
    fn to_events(&self, format: Format) -> Vec<Event<'static>>;
}

#[derive(Clone)]
pub enum CustomEvent {
    ScriptBlock(ScriptBlock),
    GraphBlock(GraphBlock),
}

impl CustomEvent {
    fn to_events(&self, format: Format) -> Vec<Event<'static>> {
        match self {
            CustomEvent::ScriptBlock(sb) => sb.to_events(format),
            CustomEvent::GraphBlock(gb) => gb.to_events(format),
        }
    }
}

#[derive(Clone)]
pub enum ExtendedEvent<'a> {
    Standard(Event<'a>),
    Custom(CustomEvent),
    Separator(u16),
}

#[derive(Copy, Clone, PartialEq)]
pub enum Format {
    Html,
    Md,
}

impl Format {
    fn transform_extended_event(self, ee: ExtendedEvent) -> Vec<Event> {
        let events = match ee {
            ExtendedEvent::Standard(e) => vec![e],
            ExtendedEvent::Custom(sb) => sb.to_events(self),
            ExtendedEvent::Separator(_) => vec![],
        };
        events
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

#[derive(Deserialize, Debug)]
pub enum CustomBlockType {
    Graph,
    Script,
    ScriptGlobals,
    Data,
    DynamicTable,
    InlineScript,
    Svg,
    Test,
}

#[derive(Deserialize, Debug)]
pub struct CustomBlockHeader {
    t: CustomBlockType,
    hidden_title: Option<String>,
}

#[derive(Debug)]
struct CustomBlockError {
    msg: String,
}

struct States {
    script: ScriptState,
    graph: GraphState,
}

impl States {
    fn new() -> Self {
        States {
            script: ScriptState::initial_state(),
            graph: GraphState::initial_state(),
        }
    }
}

fn parse_markdown(markdown: &str) -> Vec<ExtendedEvent> {
    let md_options = Options::all();

    let mut states = States::new();

    let mut current_custom_block: Option<Result<CustomBlockHeader, CustomBlockError>> = None;

    let mut level = 0;
    let mut element_i = 0;

    let parser = Parser::new_ext(markdown, md_options).map(|event| {
        match &event {
            Event::Start(_) => {
                level += 1;
                if level == 1 {
                    return vec![Event::Start(Tag::FootnoteDefinition(format!("yamdr:{}", element_i).into())), event];
                }
            }
            Event::End(_) => {
                level -= 1;
                if level == 0 {
                    element_i += 1;
                    return vec![event, Event::End(Tag::FootnoteDefinition(format!("yamdr:{}", element_i-1).into()))];
                }
            }
            _ => {}
        };
        vec![event]
    }).flatten().map(|event| match &event {
        Event::Start(Tag::FootnoteDefinition(id))
            if id.as_ref().starts_with("yamdr:") =>
        {
            vec![ExtendedEvent::Separator(str::parse(&id[6..]).unwrap())]
        }
        Event::End(Tag::FootnoteDefinition(id))
            if id.as_ref().starts_with("yamdr:") =>
        {
            Vec::new()
        }
        Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(prop)))
            if prop.as_ref().starts_with("{") =>
        {
            let Ok(block) = json::from_str::<CustomBlockHeader>(prop) else {
                current_custom_block = Some(Err(CustomBlockError{msg: "Could not parse block head".into()}));
                return Vec::new();
            };
            current_custom_block = Some(Ok(block));
            Vec::new()
        }
        Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(prop)))
            if prop.as_ref().starts_with("{") =>
        {
                current_custom_block = None;
                Vec::new()
        }
        Event::Text(text) if current_custom_block.is_some() => {
            let block = match current_custom_block.as_ref().unwrap() {
                Ok(block) => {
                    block
                },
                Err(_err) => {
                    todo!()
                    // errors.push(err.msg.clone());
                    // return vec![ExtendedEvent::Custom(CustomEvent::CustomBlockError(err.msg.clone()))]
                }
            };
            match block.t {
                CustomBlockType::Graph => {
                    match states.graph.read_block(block, text) {
                        Ok(Some(block)) => {
                            return vec![ExtendedEvent::Custom(CustomEvent::GraphBlock(block))];
                        }
                        Ok(None) => {},
                        Err(err) => {
                            println!("{}", err);
                            todo!()
                        }
                    }
                }
                CustomBlockType::DynamicTable |
                CustomBlockType::ScriptGlobals |
                CustomBlockType::Script |
                CustomBlockType::Data => {
                    match states.script.read_block(block, text) {
                        Ok(Some(block)) => {
                            return vec![ExtendedEvent::Custom(CustomEvent::ScriptBlock(block))];
                        }
                        Ok(None) => {},
                        Err(err) => {
                            println!("{}", err);
                            todo!()
                        }
                    }
                }
                _ => {}
            }
            Vec::new()
        },
        Event::Code(code) if code.starts_with("_") && code.ends_with("_") && code.len() > 2 => {
            let code = &code[1..(code.len()-1)];
            match states.script.read_block(&CustomBlockHeader{t: CustomBlockType::InlineScript, hidden_title: None}, code) {
                Ok(Some(block)) => {
                    return vec![ExtendedEvent::Custom(CustomEvent::ScriptBlock(block))];
                }
                Ok(None) => unreachable!(),
                Err(err) => {
                    println!("{}", err);
                    todo!()
                }
            }
        }
        _ => vec![ExtendedEvent::Standard(event)],
    }).flatten();

    return parser.collect();
}

pub fn render_markdown(options: &YamdrOptions, markdown: &str) -> (Meta, String) {
    let format = options.format.unwrap_or(Format::Html);

    let parser = parse_markdown(markdown).into_iter();

    let parser = parser
        .map(|ee| format.transform_extended_event(ee))
        .flatten();

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

pub struct MarkdownBlock {
    pub id: u16,
    pub html: String,
    pub markdown: String,
}

pub struct MarkdownDocumentBlocks {
    pub css: String,
    pub blocks: Vec<MarkdownBlock>,
}

impl MarkdownDocumentBlocks {
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
            let html = html.render(
                events
                    .clone()
                    .into_iter()
                    .map(|ee| html.transform_extended_event(ee))
                    .flatten(),
            );
            let markdown = md.render(
                events
                    .into_iter()
                    .map(|ee| md.transform_extended_event(ee))
                    .flatten(),
            );
            MarkdownBlock { id, html, markdown }
        })
        .collect();
    return MarkdownDocumentBlocks {
        css: STYLE.into(),
        blocks,
    };
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
"#;
        let blocks = render_blocks(document);
        assert_eq!(blocks.blocks.len(), 5);
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
