use crate::{
    plotters_block::PlottersBlock,
    utils::{dynamic_as_f64, html_hide_with_title},
    CustomBlock, CustomBlockHeader, CustomBlockReader, Error, Format, Result,
};
use pulldown_cmark::{escape::escape_html, CodeBlockKind, Event, Tag};
use rhai::{plugin::Dynamic, Engine, Scope, AST};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::RwLock;

#[derive(Debug, Clone)]
pub struct ScriptBlock {
    output: OutputType,
    header: CustomBlockHeader,
}

pub struct ScriptBlockReader {
    runtime: Runtime,
    data: BTreeMap<String, DataBlock>,
}

#[derive(Debug, Clone)]
enum OutputType {
    RunningScript(Vec<LineType>),
    Table((String, Vec<String>, Vec<Vec<String>>)),
    Inline(String),
    Data(DataBlock),
    Chart((String, Vec<Vec<(f32, f32)>>)),
}

#[derive(Debug, PartialEq, Clone)]
enum LineType {
    Code(String),
    Output(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataBlockPredefinedField {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataBlock {
    name: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    fields: Vec<DataBlockPredefinedField>,
    data: Vec<BTreeMap<String, String>>,
}

impl ScriptBlockReader {
    pub fn initial_state() -> Self {
        let engine = Engine::new();
        let scope = Scope::new();
        ScriptBlockReader {
            runtime: Runtime {
                engine,
                scope,
                globals: None,
            },
            data: BTreeMap::new(),
        }
    }
}

impl CustomBlockReader for ScriptBlockReader {
    fn can_read_block(&self, header: &CustomBlockHeader) -> bool {
        matches!(
            header.t.as_str(),
            "DynamicTable" | "DynamicChart" | "ScriptGlobals" | "Script" | "Data"
        )
    }

    fn read_block(
        &mut self,
        header: &CustomBlockHeader,
        input: &str,
    ) -> Result<Option<Box<dyn CustomBlock>>> {
        match header.t.as_str() {
            "Script" => {
                let output = self.runtime.run_block(input);
                match output {
                    Ok(output) => Ok(Some(Box::new(ScriptBlock {
                        output: OutputType::RunningScript(output),
                        header: header.clone(),
                    }))),
                    Err(err) => Err(Error::CustomBlockRead(err)),
                }
            }
            "ScriptGlobals" => {
                let output = self.runtime.add_globals(input);
                match output {
                    Ok(_) => Ok(None),
                    Err(err) => Err(Error::CustomBlockRead(err)),
                }
            }
            "DynamicTable" => match self.runtime.generate_table(input) {
                Ok((head, rows)) => Ok(Some(Box::new(ScriptBlock {
                    output: OutputType::Table((input.into(), head, rows)),
                    header: header.clone(),
                }))),
                Err(err) => Err(Error::CustomBlockRead(err)),
            },
            "DynamicChart" => match self.runtime.generate_chart(input) {
                Ok(data) => Ok(Some(Box::new(ScriptBlock {
                    output: OutputType::Chart((input.into(), data)),
                    header: header.clone(),
                }))),
                Err(err) => Err(Error::CustomBlockRead(err)),
            },
            "Data" => {
                let data: DataBlock = serde_yaml::from_str(input).map_err(|err| {
                    Error::CustomBlockRead(format!("failed to parse block: {}", err))
                })?;
                self.runtime.add_constant(data.clone());
                self.data.insert(data.name.clone(), data.clone());
                Ok(Some(Box::new(ScriptBlock {
                    output: OutputType::Data(data),
                    header: header.clone(),
                })))
            }
            _ => Err(Error::UnsupportedBlockType(header.t.clone())),
        }
    }

    fn can_read_inline(&self, inline: &str) -> bool {
        inline.len() > 3 && inline.starts_with('_') && inline.ends_with('_')
    }
    fn read_inline(&mut self, inline: &str) -> Result<Option<Box<dyn CustomBlock>>> {
        let input = &inline[1..(inline.len() - 1)];
        match self.runtime.eval_line(input) {
            Ok(output) => Ok(Some(Box::new(ScriptBlock {
                output: OutputType::Inline(format!(
                    "{} // > {}",
                    input.split(" // >").next().unwrap_or(""),
                    output
                )),
                header: CustomBlockHeader::empty("".into()),
            }))),
            Err(err) => Err(Error::CustomBlockRead(err)),
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
                            escape_html(&mut line_escaped, line).unwrap();
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

                if let Some(title) = self
                    .header
                    .fields
                    .get("hidden_title")
                    .and_then(serde_yaml::Value::as_str)
                {
                    html_hide_with_title(title.to_string(), events)
                } else {
                    events
                }
            }
            (Format::Md, OutputType::RunningScript(lines)) => {
                let props: pulldown_cmark::CowStr =
                    serde_json::to_string(&self.header).unwrap().into();
                let mut events = vec![Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(
                    props.clone(),
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
                events.push(Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(props))));

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
                            .filter(|line| !line.is_empty())
                            .map(|line| format!("// > {}", line))
                            .collect::<Vec<String>>()
                            .join("\n");
                        code += "\n";

                        let props: pulldown_cmark::CowStr =
                            serde_json::to_string(&self.header).unwrap().into();
                        vec![
                            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(props.clone()))),
                            Event::Text(code.into()),
                            Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(props))),
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
                let mut head = vec!["#".to_string()];
                head.extend(fields.keys().cloned());
                let rows: Vec<_> = data
                    .data
                    .iter()
                    .enumerate()
                    .map(|(i, data)| {
                        let mut row = vec![(i + 1).to_string()];
                        row.extend(
                            head.iter()
                                .skip(1)
                                .map(|field| data.get(field).cloned().unwrap_or_default()),
                        );
                        row
                    })
                    .collect();
                let events = build_table(&head, &rows);
                match format {
                    Format::Html => {
                        if let Some(title) = self
                            .header
                            .fields
                            .get("hidden_title")
                            .and_then(serde_yaml::Value::as_str)
                        {
                            html_hide_with_title(title.to_string(), events)
                        } else {
                            events
                        }
                    }
                    Format::Md => {
                        let table_output = crate::md::render(events.into_iter());
                        let mut output = serde_yaml::to_string(data).unwrap_or("".to_string());
                        output += "\n";
                        output += &table_output
                            .lines()
                            .filter(|line| !line.is_empty())
                            .map(|line| format!("# {}", line))
                            .collect::<Vec<String>>()
                            .join("\n");
                        output += "\n";

                        let props: pulldown_cmark::CowStr =
                            serde_json::to_string(&self.header).unwrap().into();
                        vec![
                            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(props.clone()))),
                            Event::Text(output.into()),
                            Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(props))),
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
            (Format::Html, OutputType::Chart((_, data))) => PlottersBlock::LineChart {
                title: "Todo".to_string(),
                range_x: None,
                range_y: None,
                data: data.clone(),
            }
            .to_events(Format::Html),
            _ => todo!(),
        }
    }

    #[cfg(test)]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Runtime {
    fn add_globals(&mut self, script: &str) -> Result<(), String> {
        let ast = self
            .engine
            .compile(script)
            .map_err(|err| format!("compilation error: {err:?}"))?;
        self.globals = Some(ast.clone_functions_only());
        Ok(())
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
        Ok(lines)
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

        Ok(value.to_string())
    }
    fn generate_table(&mut self, script: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
        let mut engine = Engine::new();

        let lines = Arc::new(RwLock::new(Vec::<Vec<String>>::new()));

        {
            let lines = lines.clone();
            engine.register_raw_fn(
                "row",
                [rhai::plugin::TypeId::of::<Vec<Dynamic>>()],
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
        Ok((head.pop().unwrap(), rows))
    }
    fn generate_chart(&mut self, script: &str) -> Result<Vec<Vec<(f32, f32)>>, String> {
        let mut engine = Engine::new();

        let data = Arc::new(RwLock::new(Vec::<Vec<(f32, f32)>>::new()));

        {
            let data = data.clone();
            engine.register_fn("plot", move |plot: Vec<Dynamic>| {
                data.write().unwrap().push(
                    plot.into_iter()
                        .map(|d| d.into_typed_array::<Dynamic>().unwrap())
                        .map(|v| {
                            (
                                dynamic_as_f64(&v[0]).unwrap_or(0.0) as f32,
                                dynamic_as_f64(&v[1]).unwrap_or(0.0) as f32,
                            )
                        })
                        .collect(),
                );
            });
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

        let data = data.read().unwrap().clone();
        Ok(data)
    }
    fn add_constant(&mut self, data: DataBlock) {
        let values: Vec<rhai::Dynamic> = data.data.into_iter().map(|v| v.into()).collect();
        self.scope
            .push_constant_dynamic(data.name, values.as_slice().into());
    }
}

fn build_table(head: &[String], rows: &[Vec<String>]) -> Vec<Event<'static>> {
    let mut events = Vec::new();
    events.push(Event::Start(Tag::Table(
        head.iter()
            .map(|_| pulldown_cmark::Alignment::None)
            .collect(),
    )));
    events.push(Event::Start(Tag::TableHead));
    events.extend(head.iter().flat_map(|cell| {
        vec![
            Event::Start(Tag::TableCell),
            Event::Text(cell.clone().into()),
            Event::End(Tag::TableCell),
        ]
    }));
    events.push(Event::End(Tag::TableHead));
    events.extend(rows.iter().flat_map(|row| {
        let mut events = Vec::new();
        events.push(Event::Start(Tag::TableRow));
        events.extend(row.iter().flat_map(|cell| {
            vec![
                Event::Start(Tag::TableCell),
                Event::Text(cell.clone().into()),
                Event::End(Tag::TableCell),
            ]
        }));
        events.push(Event::End(Tag::TableRow));
        events
    }));
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
    use crate::utils::custom_block_downcast;

    #[test]
    fn block_type_script() {
        let script = r#"// comment a
let x = 4 + 5; // comment b
debug(x);
debug(x + 1);
"#;
        let mut state = ScriptBlockReader::initial_state();
        let block = state.read_block(&CustomBlockHeader::empty("Script".into()), script);
        let block: ScriptBlock = custom_block_downcast(block.unwrap().unwrap())
            .expect("block should be type ScriptBlock");
        let lines = if let OutputType::RunningScript(lines) = block.output {
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
        let script = r#"_4 + 5_"#;
        let mut state = ScriptBlockReader::initial_state();
        let block = state.read_inline(script);
        let block: ScriptBlock = custom_block_downcast(block.unwrap().unwrap())
            .expect("block should be type ScriptBlock");
        let line = if let OutputType::Inline(line) = block.output {
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
        let mut state = ScriptBlockReader::initial_state();
        let block = state.read_block(&CustomBlockHeader::empty("ScriptGlobals".into()), globals);
        assert!(block.unwrap().is_none(), "output should be None");

        let script = r#"_test(5)_"#;
        let block = state.read_inline(script);
        let block: ScriptBlock = custom_block_downcast(block.unwrap().unwrap())
            .expect("block should be type ScriptBlock");
        let line = if let OutputType::Inline(line) = block.output {
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
        let mut state = ScriptBlockReader::initial_state();
        let _ = state.read_block(&CustomBlockHeader::empty("Script".into()), script);

        let script = r#"_x + 1_"#;
        let block = state.read_inline(script);
        let block: ScriptBlock = custom_block_downcast(block.unwrap().unwrap())
            .expect("block should be type ScriptBlock");
        let line = if let OutputType::Inline(line) = block.output {
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
        let mut state = ScriptBlockReader::initial_state();
        let block = state.read_block(&CustomBlockHeader::empty("DynamicTable".into()), script);
        let block: ScriptBlock =
            custom_block_downcast(block.unwrap().unwrap()).expect("block should be ScriptBlock");
        let (head, rows) = if let OutputType::Table((_, head, rows)) = block.output {
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
        let mut state = ScriptBlockReader::initial_state();
        state
            .read_block(&CustomBlockHeader::empty("Data".into()), data)
            .unwrap();

        let script = r#"_testdata[1]["fieldA"]_"#;
        let block = state.read_inline(script);
        let block: ScriptBlock =
            custom_block_downcast(block.unwrap().unwrap()).expect("block should be ScriptBlock");
        let line = if let OutputType::Inline(line) = block.output {
            line
        } else {
            panic!("output type should be OutputType::Inline");
        };
        assert_eq!(line, r#"testdata[1]["fieldA"] // > 3"#);
    }

    #[test]
    fn block_type_dynamic_chart() {
        let script = r#"
plot([[0, 0], [2, 1], [4, 2]]);
plot([[4, 2], [2, 3], [0, 4]]);
"#;
        let mut state = ScriptBlockReader::initial_state();
        let block = state.read_block(&CustomBlockHeader::empty("DynamicChart".into()), script);
        let block: ScriptBlock =
            custom_block_downcast(block.unwrap().unwrap()).expect("block should be ScriptBlock");
        let data = if let OutputType::Chart((_, data)) = block.output {
            data
        } else {
            panic!("output type should be OutputType::Chart");
        };
        assert_eq!(data.len(), 2, "there should be 2 sets of data");
        assert_eq!(data[0], &[(0.0, 0.0), (2.0, 1.0), (4.0, 2.0)]);
        assert_eq!(data[1], &[(4.0, 2.0), (2.0, 3.0), (0.0, 4.0)]);
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
                r#"```{"t":"Script"}
let x = 1 + 1;
```

"#,
                r#"```{"t":"Script"}
let x = 1 + 1;
```

"#,
            ),
            (
                r#"```{"t":"Script"}
let x = 1 + 1;
// Existing "output comment" should be stripped
debug(x);
// > 123
```

"#,
                r#"```{"t":"Script"}
let x = 1 + 1;
// Existing "output comment" should be stripped
debug(x);
// > 2
```

"#,
            ),
            (
                r#"```{"t":"DynamicTable"}
row([1,2,3,4]);
row([1,2,3,4]);
row([1,2,3,4]);
```

"#,
                r#"```{"t":"DynamicTable"}
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
                r#"```{"t":"DynamicTable"}
row([1,2,3,4]);
row([1,2,3,4]);
row([1,2,3,4]);
// > | 1 | 2 | 3 | 4 |
// > |---|---|---|---|
// > | "output comments" should be stripped | 2 | 3 | 4 |
// > | 1 | 2 | 3 | 4 |
```

"#,
                r#"```{"t":"DynamicTable"}
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
                r#"```{"t":"Data"}
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
                r#"```{"t":"Data"}
name: test
data:
- a: '1'
  b: '2'
- a: '3'
  c: '4'
- a: '5'
  b: '6'
  c: '7'

# | # | a | b | c |
# |---|---|---|---|
# | 1 | 1 | 2 |  |
# | 2 | 3 |  | 4 |
# | 3 | 5 | 6 | 7 |
```

"#,
            ),
            (
                r#"```{"t":"Script","hidden_title":"abc"}
let x = 1 + 1;
```

"#,
                r#"```{"t":"Script","hidden_title":"abc"}
let x = 1 + 1;
```

"#,
            ),
        ];
        let format = crate::Format::Md;
        for (document, expected) in documents {
            let parsed_markdown = crate::parse_markdown(document);
            let events = parsed_markdown
                .iter()
                .flat_map(|ee| format.transform_extended_event(ee));
            let output = format.render(events);

            println!("Wanted:\n{}\nGot:\n{}", expected, output);

            assert_eq!(expected, output);

            let parsed_markdown = crate::parse_markdown(document);
            let events = parsed_markdown
                .iter()
                .flat_map(|ee| format.transform_extended_event(ee));
            let output = format.render(events);

            assert_eq!(
                expected, output,
                "output rendered again should produce the same output"
            );
        }
    }
}
