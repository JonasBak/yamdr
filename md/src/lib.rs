use layout::backends::svg::SVGWriter;
use layout::gv;
use miniserde::{json, Deserialize};
use pulldown_cmark::{escape::escape_html, html, CodeBlockKind, Event, Options, Parser, Tag};

mod script {
    use pulldown_cmark::escape::escape_html;
    use rhai::{plugin::Dynamic, Engine, Scope};

    pub fn run_block(script: String) -> String {
        let engine = Engine::new();
        let mut scope = Scope::new();

        let mut output = String::new();

        for line in script.lines() {
            let mut line_escaped = String::new();
            escape_html(&mut line_escaped, &line).unwrap();
            output += &format!(r#"<span class="script-code">{}</span>"#, line_escaped);
            output += "\n";
            let result = engine.eval_with_scope::<Dynamic>(&mut scope, line).unwrap();
            if !result.is::<()>() {
                let mut result_escaped = String::new();
                escape_html(&mut result_escaped, &format!("> {:?}", result)).unwrap();
                output += &format!(r#"<span class="script-output">{}</span>"#, result_escaped);
                output += "\n";
            }
        }
        return output;
    }
}

static STYLE: &str = r#"
<style>
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
</style>
"#;

pub struct StandaloneOptions {}

pub struct YamdrOptions {
    pub standalone: Option<StandaloneOptions>,
}

pub struct Meta {}

#[derive(Deserialize, Debug)]
enum CustomBlockType {
    Graph,
    Script,
    Test,
}

#[derive(Deserialize, Debug)]
struct CustomBlock {
    t: CustomBlockType,
}

#[derive(Debug)]
struct CustomBlockError {
    msg: String,
}

fn error_event<'a>(msg: &str) -> Event<'a> {
    let mut escaped = "".to_string();
    escape_html(&mut escaped, msg).unwrap();
    let tag = format!(r#"<div class="error">{}</div>"#, escaped);
    return Event::Html(tag.into());
}

pub fn markdown_to_html(options: YamdrOptions, markdown: &str) -> (Meta, String) {
    let md_options = Options::all();
    let parser = Parser::new_ext(markdown, md_options);

    let mut current_custom_block: Option<Result<CustomBlock, CustomBlockError>> = None;

    let mut errors = Vec::new();

    let parser = parser.filter_map(|event| match &event {
        Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(prop)))
            if prop.as_ref().starts_with("{") =>
        {
            let Ok(block) = json::from_str::<CustomBlock>(prop) else {
                current_custom_block = Some(Err(CustomBlockError{msg: "Could not parse block head".into()}));
                return None;
            };
            current_custom_block = Some(Ok(block));
            None
        }
        Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(prop)))
            if prop.as_ref().starts_with("{") =>
        {
                current_custom_block = None;
                None
        }
        Event::Text(text) if current_custom_block.is_some() => {
            let block = match current_custom_block.as_ref().unwrap() {
                Ok(block) => {
                    block
                },
                Err(err) => {
                    errors.push(err.msg.clone());
                    return Some(error_event(&err.msg));
                }
            };
            match block.t {
                CustomBlockType::Graph => {
                    match gv::DotParser::new(text).process() {
                        Ok(g) => {
                            let mut gb = gv::GraphBuilder::new();
                            gb.visit_graph(&g);
                            let mut graph = gb.get();
                            let mut svg = SVGWriter::new();
                            graph.do_it(
                                false,
                                false,
                                false,
                                &mut svg,
                            );
                            let content = svg.finalize();
                            return Some(Event::Html(content.into()));
                        }
                        Err(err) => {
                            let msg = format!("error parsing graph block: {}", err);
                            errors.push(msg.clone());
                            return Some(error_event(&msg));
                        }
                    }
                }
                CustomBlockType::Script => {
                    let output = script::run_block(text.to_string());
                    let tag = format!(r#"<div class="script"><pre>{}</pre></div>"#, output);
                    return Some(Event::Html(tag.into()));
                }
                _ => {}
            }
            None
        },
        _ => Some(event),
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    if let Some(standalone_options) = options.standalone {
        html_output = format!(
            r#"
<!DOCTYPE html>
<html>
    <head>
        {}
    </head>
    <body>
        <div class="content">
            {}
        </div>
    </body>
</html>"#,
            STYLE, html_output
        );
    } else {
        html_output = format!(
            r#"
{}
<div class="content">
{}
</div>"#,
            STYLE, html_output
        );
    }

    let meta = Meta {};

    (meta, html_output)
}
