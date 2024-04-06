use crate::{CustomBlock, CustomBlockHeader, CustomBlockReader, Error, Format, Result};
use layout::backends::svg::SVGWriter;
use layout::gv;
use pulldown_cmark::{CodeBlockKind, Event, Tag};

#[derive(Debug, Clone)]
pub struct GraphBlock {
    input: String,
    output: String,
}

pub struct GraphBlockReader {}

impl GraphBlockReader {
    pub fn initial_state() -> Self {
        GraphBlockReader {}
    }
}

impl CustomBlockReader for GraphBlockReader {
    fn can_read_block(&self, header: &CustomBlockHeader) -> bool {
        header.t == "Graph"
    }

    fn read_block(
        &mut self,
        _header: &CustomBlockHeader,
        input: &str,
    ) -> Result<Option<Box<dyn CustomBlock>>> {
        match gv::DotParser::new(input).process() {
            Ok(g) => {
                let mut gb = gv::GraphBuilder::new();
                gb.visit_graph(&g);
                let mut graph = gb.get();
                let mut svg = SVGWriter::new();
                graph.do_it(false, false, false, &mut svg);
                let output = svg.finalize();
                Ok(Some(Box::new(GraphBlock {
                    input: input.into(),
                    output,
                })))
            }
            Err(err) => Err(Error::CustomBlockRead(err)),
        }
    }
}

impl CustomBlock for GraphBlock {
    fn to_events(&self, format: Format) -> Vec<Event<'static>> {
        match format {
            Format::Html => {
                vec![Event::Html(self.output.clone().into())]
            }
            Format::Md => {
                let props = r#"{"t": "Graph"}"#;
                vec![
                    Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(props.into()))),
                    Event::Text(self.input.clone().into()),
                    Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(props.into()))),
                ]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn render_markdown() {
        let documents = [(
            r#"```{"t": "Graph"}
digraph D {

  A;
  B;
  C;
  D;
  E;

  A -- B;
  A -- C;
  A -- D;
  B -- E;
  C -- E;
  D -- E;

}
```

"#,
            r#"```{"t": "Graph"}
digraph D {

  A;
  B;
  C;
  D;
  E;

  A -- B;
  A -- C;
  A -- D;
  B -- E;
  C -- E;
  D -- E;

}
```

"#,
        )];
        let format = crate::Format::Md;
        for (document, expected) in documents {
            let parsed_markdown = crate::parse_markdown(document);
            let events = parsed_markdown
                .iter()
                .flat_map(|ee| format.transform_extended_event(ee));
            let output = format.render(events);

            assert_eq!(expected, output);
        }
    }
}
