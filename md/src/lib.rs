use layout::backends::svg::SVGWriter;
use layout::gv;
use miniserde::{json, Deserialize};
use pulldown_cmark::{escape::escape_html, html, CodeBlockKind, Event, Options, Parser, Tag};

mod script {
    use pulldown_cmark::escape::escape_html;
    use rhai::{plugin::Dynamic, Engine, Scope, AST};
    use std::sync::Arc;
    use std::sync::RwLock;

    pub struct Runtime<'a> {
        engine: Engine,
        scope: Scope<'a>,
        globals: Option<AST>,
    }

    impl Runtime<'_> {
        pub fn new() -> Self {
            let mut engine = Engine::new();
            let mut scope = Scope::new();
            return Runtime {
                engine,
                scope,
                globals: None,
            };
        }
        pub fn add_globals(&mut self, script: &str) -> Result<(), String> {
            let ast = self
                .engine
                .compile(script)
                .map_err(|err| format!("compilation error: {err:?}"))?;
            self.globals = Some(ast.clone_functions_only());
            return Ok(());
        }
        pub fn run_block(&mut self, script: &str) -> Result<String, String> {
            let mut output = String::new();

            let mut printed = String::new();

            let logbook = Arc::new(RwLock::new(Vec::<(usize, String)>::new()));

            let log = logbook.clone();
            self.engine.on_debug(move |s, _, pos| {
                log.write()
                    .unwrap()
                    .push((pos.line().unwrap_or(1), s.to_string()));
            });

            let mut ast = self
                .engine
                .compile(script)
                .map_err(|err| format!("compilation error: {err:?}"))?;

            if let Some(globals) = self.globals.as_ref() {
                ast = globals.merge(&ast);
            }

            self.engine
                .run_ast_with_scope(&mut self.scope, &ast)
                .map_err(|err| format!("runtime error: {err:?}"))?;

            for (i, line) in script.lines().enumerate() {
                let mut line_escaped = String::new();
                escape_html(&mut line_escaped, &line).unwrap();
                output += &format!(r#"<span class="script-code">{}</span>"#, line_escaped);
                output += "\n";
                for (_, entry) in logbook.read().unwrap().iter().filter(|(l, _)| *l == i + 1) {
                    let mut entry_escaped = String::new();
                    escape_html(&mut entry_escaped, &format!("// > {entry}")).unwrap();
                    output += &format!(r#"<span class="script-output">{}</span>"#, entry_escaped);
                    output += "\n";
                }
            }
            return Ok(output);
        }
        pub fn generate_table(
            &mut self,
            script: &str,
        ) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
            let mut engine = Engine::new();

            let lines = Arc::new(RwLock::new(Vec::<Vec<String>>::new()));

            {
                let lines = lines.clone();
                engine.register_raw_fn(
                    "row",
                    &[rhai::plugin::TypeId::of::<Vec<Dynamic>>()],
                    move |_, args| {
                        lines.write().unwrap().push(
                            args[0]
                                .clone()
                                .into_typed_array::<Dynamic>()
                                .unwrap()
                                .iter()
                                .map(|arg| format!("{arg:?}"))
                                .collect(),
                        );
                        Ok(())
                    },
                );
            }

            let mut ast = engine
                .compile(script)
                .map_err(|err| format!("compilation error: {err:?}"))?;

            if let Some(globals) = self.globals.as_ref() {
                ast = globals.merge(&ast);
            }

            engine
                .run_ast_with_scope(&mut self.scope, &ast)
                .map_err(|err| format!("runtime error: {err:?}"))?;

            let mut head = lines.read().unwrap().clone();
            let rows = head.split_off(1);
            return Ok((head.pop().unwrap(), rows));
        }
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
    ScriptGlobals,
    DynamicTable,
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

    let mut runtime = script::Runtime::new();

    let mut current_custom_block: Option<Result<CustomBlock, CustomBlockError>> = None;

    let mut errors = Vec::new();

    let parser = parser.map(|event| match &event {
        Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(prop)))
            if prop.as_ref().starts_with("{") =>
        {
            let Ok(block) = json::from_str::<CustomBlock>(prop) else {
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
                Err(err) => {
                    errors.push(err.msg.clone());
                    return vec![error_event(&err.msg)]
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
                            return vec![Event::Html(content.into())];
                        }
                        Err(err) => {
                            let msg = format!("error parsing graph block: {}", err);
                            errors.push(msg.clone());
                            return vec![error_event(&msg)];
                        }
                    }
                }
                CustomBlockType::Script => {
                    let output = runtime.run_block(text);
                    match output {
                        Ok(output) => {
                            let tag = format!(r#"<div class="script"><pre>{}</pre></div>"#, output);
                            return vec![Event::Html(tag.into())];
                        }
                        Err(err) => {
                            errors.push(err.clone());
                            return vec![error_event(&err)];
                        }
                    }
                }
                CustomBlockType::ScriptGlobals => {
                    let output = runtime.add_globals(text);
                    match output {
                        Ok(_) => {
                            return Vec::new();
                        }
                        Err(err) => {
                            errors.push(err.clone());
                            return vec![error_event(&err)];
                        }
                    }
                }
                CustomBlockType::DynamicTable => {
                    let rows = runtime.generate_table(text);
                    match rows {
                        Ok((head, rows)) => {
                            let mut events = Vec::new();
                            events.push(Event::Start(Tag::Table(head.iter().map(|_| pulldown_cmark::Alignment::None).collect())));
                            events.push(Event::Start(Tag::TableHead));
                            events.extend(head.iter().map(|cell|  vec![
                                Event::Start(Tag::TableCell),
                                Event::Text(cell.clone().into()),
                                Event::End(Tag::TableCell),
                            ]).flatten());
                            events.push(Event::End(Tag::TableHead));
                            events.extend(rows.into_iter().enumerate().map(|(i, row)| {
                                let mut events = Vec::new();
                                events.push(Event::Start(Tag::TableRow));
                                events.extend(row.iter().map(|cell|  vec![
                                    Event::Start(Tag::TableCell),
                                    Event::Text(cell.clone().into()),
                                    Event::End(Tag::TableCell),
                                ]).flatten());
                                events.push(Event::End(Tag::TableRow));
                                return events;
                            }).flatten());
                            events.push(Event::End(Tag::Table(head.iter().map(|_| pulldown_cmark::Alignment::None).collect())));
                            return events;
                        }
                        Err(err) => {
                            errors.push(err.clone());
                            return vec![error_event(&err)];
                        }
                    }
                }
                _ => {}
            }
            Vec::new()
        },
        _ => vec![event],
    }).flatten();

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
