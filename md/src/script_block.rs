use crate::{CustomBlock, CustomBlockHeader, CustomBlockState, CustomBlockType, Format};
use pulldown_cmark::{escape::escape_html, CodeBlockKind, Event, Tag};
use rhai::{plugin::Dynamic, Engine, Scope, AST};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::RwLock;

#[derive(Debug, Clone)]
pub struct ScriptBlock {
    hidden_title: Option<String>,
    output: OutputType,
}

pub struct ScriptState {
    runtime: Runtime,
}

#[derive(Debug, Clone)]
enum OutputType {
    RunningScript(Vec<LineType>),
    Table((String, Vec<String>, Vec<Vec<String>>)),
    Inline(String),
    Data(DataBlock),
}

#[derive(Debug, PartialEq, Clone)]
enum LineType {
    Code(String),
    Output(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataBlock {
    name: String,
    data: Vec<BTreeMap<String, String>>,
}

impl CustomBlockState for ScriptState {
    type Block = ScriptBlock;

    fn initial_state() -> Self {
        let engine = Engine::new();
        let scope = Scope::new();
        return ScriptState {
            runtime: Runtime {
                engine,
                scope,
                globals: None,
            },
        };
    }

    fn read_block(
        &mut self,
        header: &CustomBlockHeader,
        input: &str,
    ) -> Result<Option<Self::Block>, String> {
        match header.t {
            CustomBlockType::Script => {
                let output = self.runtime.run_block(input);
                match output {
                    Ok(output) => Ok(Some(Self::Block {
                        hidden_title: header.hidden_title.clone(),
                        output: OutputType::RunningScript(output),
                    })),
                    Err(err) => Err(err),
                }
            }
            CustomBlockType::ScriptGlobals => {
                let output = self.runtime.add_globals(input);
                match output {
                    Ok(_) => return Ok(None),
                    Err(err) => return Err(err),
                }
            }
            CustomBlockType::DynamicTable => match self.runtime.generate_table(input) {
                Ok((head, rows)) => Ok(Some(Self::Block {
                    hidden_title: header.hidden_title.clone(),
                    output: OutputType::Table((input.into(), head, rows)),
                })),
                Err(err) => Err(err),
            },
            CustomBlockType::InlineScript => match self.runtime.eval_line(input) {
                Ok(output) => Ok(Some(Self::Block {
                    hidden_title: None,
                    output: OutputType::Inline(format!(
                        "{} // > {}",
                        input.split(" // >").next().unwrap_or(""),
                        output
                    )),
                })),
                Err(err) => Err(err),
            },
            CustomBlockType::Data => {
                let data: DataBlock = serde_yaml::from_str(input)
                    .map_err(|err| format!("failed to parse block: {}", err.to_string()))?;
                let output = self.runtime.add_constant(data.clone());
                Ok(Some(Self::Block {
                    hidden_title: None,
                    output: OutputType::Data(data),
                }))
            }
            _ => {
                return Err(format!(
                    "Unsupported block type for ScriptBlock: {:?}",
                    header.t
                ))
            }
        }
    }
}

struct Runtime {
    engine: Engine,
    scope: Scope<'static>,
    globals: Option<AST>,
}

impl CustomBlock for ScriptBlock {
    fn to_events(&self, format: Format) -> Vec<Event<'static>> {
        match (format, &self.output) {
            (Format::Html, OutputType::RunningScript(lines)) => {
                let mut events = vec![Event::Html(r#"<div class="script"><pre>"#.into())];
                for line in lines {
                    let escaped = match line {
                        LineType::Code(line) => {
                            let mut line_escaped = String::new();
                            escape_html(&mut line_escaped, &line).unwrap();
                            format!(r#"<span class="script-code">{}</span>"#, line_escaped) + "\n"
                        }
                        LineType::Output(line) => {
                            let mut line_escaped = String::new();
                            escape_html(&mut line_escaped, &format!("// > {}", line)).unwrap();
                            format!(r#"<span class="script-output">{}</span>"#, line_escaped) + "\n"
                        }
                    };
                    events.push(Event::Html(escaped.into()));
                }
                events.push(Event::Html(r#"</pre></div>"#.into()));

                events
            }
            (Format::Md, OutputType::RunningScript(lines)) => {
                let props = r#"{"t": "Script"}"#;
                let mut events = vec![Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(
                    props.into(),
                )))];
                let mut code = "".to_string();
                for line in lines {
                    match line {
                        LineType::Code(line) => {
                            code += line;
                        }
                        LineType::Output(line) => {
                            code += &format!("// > {}", line);
                        }
                    };
                    code += "\n";
                }
                events.push(Event::Text(code.into()));
                events.push(Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(
                    props.into(),
                ))));

                events
            }
            (format, OutputType::Table((code, head, rows))) => {
                let events = build_table(head, rows);
                match format {
                    Format::Html => events,
                    Format::Md => {
                        let table_output = crate::md::render(events.into_iter());
                        let mut code = code
                            .lines()
                            .filter(|line| !line.starts_with("// > "))
                            .collect::<Vec<&str>>()
                            .join("\n");
                        code += "\n";
                        code += &table_output
                            .lines()
                            .filter(|line| line.len() > 0)
                            .map(|line| format!("// > {}", line))
                            .collect::<Vec<String>>()
                            .join("\n");
                        code += "\n";

                        let props = r#"{"t": "DynamicTable"}"#;
                        vec![
                            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(props.into()))),
                            Event::Text(code.into()),
                            Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(props.into()))),
                        ]
                    }
                }
            }
            (format, OutputType::Data(data)) => {
                let mut fields = BTreeMap::new();
                for data in &data.data {
                    for field in data.keys() {
                        fields.insert(field.clone(), true);
                    }
                }
                let head: Vec<_> = fields.keys().cloned().collect();
                let rows = data
                    .data
                    .iter()
                    .map(|data| {
                        head.iter()
                            .map(|field| data.get(field).cloned().unwrap_or_default())
                            .collect()
                    })
                    .collect();
                let events = build_table(&head, &rows);
                match format {
                    Format::Html => events,
                    Format::Md => {
                        let table_output = crate::md::render(events.into_iter());
                        let mut output = serde_yaml::to_string(data).unwrap_or("".to_string());
                        output += "\n";
                        output += &table_output
                            .lines()
                            .filter(|line| line.len() > 0)
                            .map(|line| format!("# {}", line))
                            .collect::<Vec<String>>()
                            .join("\n");
                        output += "\n";

                        let props = r#"{"t": "Data"}"#;
                        vec![
                            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(props.into()))),
                            Event::Text(output.into()),
                            Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(props.into()))),
                        ]
                    }
                }
            }
            (Format::Html, OutputType::Inline(output)) => {
                let mut escaped = "".to_string();
                escape_html(&mut escaped, output).unwrap();
                vec![Event::Html(
                    format!(r#"<code class="inline-script">{}</code>"#, escaped).into(),
                )]
            }
            (Format::Md, OutputType::Inline(output)) => {
                vec![Event::Code(format!(r#"_{}_"#, output).into())]
            }
            _ => todo!(),
        }
    }
}

impl Runtime {
    fn new() -> Self {
        let engine = Engine::new();
        let scope = Scope::new();
        return Runtime {
            engine,
            scope,
            globals: None,
        };
    }
    fn add_globals(&mut self, script: &str) -> Result<(), String> {
        let ast = self
            .engine
            .compile(script)
            .map_err(|err| format!("compilation error: {err:?}"))?;
        self.globals = Some(ast.clone_functions_only());
        return Ok(());
    }
    fn run_block(&mut self, script: &str) -> Result<Vec<LineType>, String> {
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

        let mut lines = Vec::new();

        for (i, line) in script.lines().enumerate() {
            lines.push(LineType::Code(line.into()));
            for (_, entry) in logbook.read().unwrap().iter().filter(|(l, _)| *l == i + 1) {
                lines.push(LineType::Output(entry.into()));
            }
        }

        let lines = lines
            .into_iter()
            .filter(|line| match line {
                LineType::Output(_) => true,
                LineType::Code(line) => !line.starts_with("// > "),
            })
            .collect();
        return Ok(lines);
    }
    fn eval_line(&mut self, script: &str) -> Result<String, String> {
        let mut ast = self
            .engine
            .compile(script)
            .map_err(|err| format!("compilation error: {err:?}"))?;

        if let Some(globals) = self.globals.as_ref() {
            ast = globals.merge(&ast);
        }

        let value = self
            .engine
            .eval_ast_with_scope::<Dynamic>(&mut self.scope, &ast)
            .map_err(|err| format!("runtime error: {err:?}"))?;

        return Ok(value.to_string());
    }
    fn generate_table(&mut self, script: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
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
                            .map(|arg| arg.to_string())
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
    fn add_constant(&mut self, data: DataBlock) {
        let values: Vec<rhai::Dynamic> = data.data.into_iter().map(|v| v.into()).collect();
        self.scope
            .push_constant_dynamic(data.name, values.as_slice().into());
    }
}

fn build_table(head: &Vec<String>, rows: &Vec<Vec<String>>) -> Vec<Event<'static>> {
    let mut events = Vec::new();
    events.push(Event::Start(Tag::Table(
        head.iter()
            .map(|_| pulldown_cmark::Alignment::None)
            .collect(),
    )));
    events.push(Event::Start(Tag::TableHead));
    events.extend(
        head.iter()
            .map(|cell| {
                vec![
                    Event::Start(Tag::TableCell),
                    Event::Text(cell.clone().into()),
                    Event::End(Tag::TableCell),
                ]
            })
            .flatten(),
    );
    events.push(Event::End(Tag::TableHead));
    events.extend(
        rows.into_iter()
            .map(|row| {
                let mut events = Vec::new();
                events.push(Event::Start(Tag::TableRow));
                events.extend(
                    row.iter()
                        .map(|cell| {
                            vec![
                                Event::Start(Tag::TableCell),
                                Event::Text(cell.clone().into()),
                                Event::End(Tag::TableCell),
                            ]
                        })
                        .flatten(),
                );
                events.push(Event::End(Tag::TableRow));
                return events;
            })
            .flatten(),
    );
    events.push(Event::End(Tag::Table(
        head.iter()
            .map(|_| pulldown_cmark::Alignment::None)
            .collect(),
    )));
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};

    #[test]
    fn block_type_script() {
        let script = r#"// comment a
let x = 4 + 5; // comment b
debug(x);
debug(x + 1);
"#;
        let mut state = ScriptState::initial_state();
        let block = state.read_block(
            &CustomBlockHeader {
                t: CustomBlockType::Script,
                hidden_title: None,
            },
            script,
        );
        let lines = if let OutputType::RunningScript(lines) = block.unwrap().unwrap().output {
            lines
        } else {
            panic!("output type should be OutputType::RunningScript");
        };
        assert_eq!(lines.len(), 6, "output should be 6 lines");
        assert_eq!(lines[0], LineType::Code("// comment a".into()));
        assert_eq!(
            lines[1],
            LineType::Code("let x = 4 + 5; // comment b".into())
        );
        assert_eq!(lines[2], LineType::Code("debug(x);".into()));
        assert_eq!(lines[3], LineType::Output("9".into()));
        assert_eq!(lines[4], LineType::Code("debug(x + 1);".into()));
        assert_eq!(lines[5], LineType::Output("10".into()));
    }

    #[test]
    fn block_type_inline_script() {
        let script = r#"4 + 5"#;
        let mut state = ScriptState::initial_state();
        let block = state.read_block(
            &CustomBlockHeader {
                t: CustomBlockType::InlineScript,
                hidden_title: None,
            },
            script,
        );
        let line = if let OutputType::Inline(line) = block.unwrap().unwrap().output {
            line
        } else {
            panic!("output type should be OutputType::Inline");
        };
        assert_eq!(line, "4 + 5 // > 9");
    }

    #[test]
    fn block_type_script_globals() {
        let globals = r#"
fn test(n) {
    n + 1
}
"#;
        let mut state = ScriptState::initial_state();
        let block = state.read_block(
            &CustomBlockHeader {
                t: CustomBlockType::ScriptGlobals,
                hidden_title: None,
            },
            globals,
        );
        assert!(block.unwrap().is_none(), "output should be None");

        let script = r#"test(5)"#;
        let block = state.read_block(
            &CustomBlockHeader {
                t: CustomBlockType::InlineScript,
                hidden_title: None,
            },
            script,
        );
        let line = if let OutputType::Inline(line) = block.unwrap().unwrap().output {
            line
        } else {
            panic!("output type should be OutputType::Inline");
        };
        assert_eq!(line, "test(5) // > 6");
    }

    #[test]
    fn values_persist() {
        let script = r#"
let x = 5;
"#;
        let mut state = ScriptState::initial_state();
        let _ = state.read_block(
            &CustomBlockHeader {
                t: CustomBlockType::Script,
                hidden_title: None,
            },
            script,
        );

        let script = r#"x + 1"#;
        let block = state.read_block(
            &CustomBlockHeader {
                t: CustomBlockType::InlineScript,
                hidden_title: None,
            },
            script,
        );
        let line = if let OutputType::Inline(line) = block.unwrap().unwrap().output {
            line
        } else {
            panic!("output type should be OutputType::Inline");
        };
        assert_eq!(line, "x + 1 // > 6");
    }

    #[test]
    fn block_type_dynamic_table() {
        let script = r#"
row(["head A", "head B", "head C"]);
row([1, 2, 3]);
row([4, 5, 6]);
row([7, 8, 9]);
"#;
        let mut state = ScriptState::initial_state();
        let block = state.read_block(
            &CustomBlockHeader {
                t: CustomBlockType::DynamicTable,
                hidden_title: None,
            },
            script,
        );
        let (head, rows) =
            if let OutputType::Table((_, head, rows)) = block.unwrap().unwrap().output {
                (head, rows)
            } else {
                panic!("output type should be OutputType::Table");
            };
        assert_eq!(head, &["head A", "head B", "head C"]);
        assert_eq!(rows.len(), 3, "there should be 3 rows");
        assert_eq!(rows[0], &["1", "2", "3"]);
        assert_eq!(rows[1], &["4", "5", "6"]);
        assert_eq!(rows[2], &["7", "8", "9"]);
    }

    #[test]
    fn block_type_data() {
        let data = r#"
name: testdata
data:
- fieldA: 1
  fieldB: 2
- fieldA: 3
  fieldB: 4
"#;
        let mut state = ScriptState::initial_state();
        state
            .read_block(
                &CustomBlockHeader {
                    t: CustomBlockType::Data,
                    hidden_title: None,
                },
                data,
            )
            .unwrap();

        let script = r#"testdata[1]["fieldA"]"#;
        let block = state.read_block(
            &CustomBlockHeader {
                t: CustomBlockType::InlineScript,
                hidden_title: None,
            },
            script,
        );
        let line = if let OutputType::Inline(line) = block.unwrap().unwrap().output {
            line
        } else {
            panic!("output type should be OutputType::Inline");
        };
        assert_eq!(line, r#"testdata[1]["fieldA"] // > 3"#);
    }

    #[test]
    fn render_markdown() {
        let documents = [
            (
                r#"Some inline code `_3 + 1_`."#,
                r#"Some inline code `_3 + 1 // > 4_`.

"#,
            ),
            (
                r#"inline code with "output comment" strips comment `_3 + 1 // > abc_`."#,
                //                  v              v Quotes are changed by pulldown_cmark
                r#"inline code with “output comment” strips comment `_3 + 1 // > 4_`.

"#,
            ),
            (
                r#"```{"t": "Script"}
let x = 1 + 1;
```

"#,
                r#"```{"t": "Script"}
let x = 1 + 1;
```

"#,
            ),
            (
                r#"```{"t": "Script"}
let x = 1 + 1;
// Existing "output comment" should be stripped
debug(x);
// > 123
```

"#,
                r#"```{"t": "Script"}
let x = 1 + 1;
// Existing "output comment" should be stripped
debug(x);
// > 2
```

"#,
            ),
            (
                r#"```{"t": "DynamicTable"}
row([1,2,3,4]);
row([1,2,3,4]);
row([1,2,3,4]);
```

"#,
                r#"```{"t": "DynamicTable"}
row([1,2,3,4]);
row([1,2,3,4]);
row([1,2,3,4]);
// > | 1 | 2 | 3 | 4 |
// > |---|---|---|---|
// > | 1 | 2 | 3 | 4 |
// > | 1 | 2 | 3 | 4 |
```

"#,
            ),
            (
                r#"```{"t": "DynamicTable"}
row([1,2,3,4]);
row([1,2,3,4]);
row([1,2,3,4]);
// > | 1 | 2 | 3 | 4 |
// > |---|---|---|---|
// > | "output comments" should be stripped | 2 | 3 | 4 |
// > | 1 | 2 | 3 | 4 |
```

"#,
                r#"```{"t": "DynamicTable"}
row([1,2,3,4]);
row([1,2,3,4]);
row([1,2,3,4]);
// > | 1 | 2 | 3 | 4 |
// > |---|---|---|---|
// > | 1 | 2 | 3 | 4 |
// > | 1 | 2 | 3 | 4 |
```

"#,
            ),
            (
                r#"```{"t": "Data"}
name: test
data:
- a: 1
  b: 2
- a: 3
  c: 4
- a: 5
  b: 6
  c: 7
```

"#,
                r#"```{"t": "Data"}
name: test
data:
- a: '1'
  b: '2'
- a: '3'
  c: '4'
- a: '5'
  b: '6'
  c: '7'

# | a | b | c |
# |---|---|---|
# | 1 | 2 |  |
# | 3 |  | 4 |
# | 5 | 6 | 7 |
```

"#,
            ),
        ];
        let format = crate::Format::Md;
        for (document, expected) in documents {
            let events = crate::parse_markdown(document)
                .into_iter()
                .map(|ee| format.transform_extended_event(ee))
                .flatten();
            let output = format.render(events);

            println!("Wanted:\n{}\nGot:\n{}", expected, output);

            assert_eq!(expected, output);
        }
    }
}
