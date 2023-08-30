use crate::{CustomBlock, CustomBlockHeader, CustomBlockState, CustomBlockType, Format};
use layout::backends::svg::SVGWriter;
use layout::gv;
use pulldown_cmark::{escape::escape_html, CodeBlockKind, Event, Tag};

#[derive(Debug, Clone)]
pub struct GraphBlock {
    input: String,
    output: String,
}

pub struct GraphState {}

impl CustomBlockState for GraphState {
    type Block = GraphBlock;

    fn initial_state() -> Self {
        return GraphState {};
    }

    fn read_block(
        &mut self,
        header: &CustomBlockHeader,
        input: &str,
    ) -> Result<Option<Self::Block>, String> {
        match gv::DotParser::new(input).process() {
            Ok(g) => {
                let mut gb = gv::GraphBuilder::new();
                gb.visit_graph(&g);
                let mut graph = gb.get();
                let mut svg = SVGWriter::new();
                graph.do_it(false, false, false, &mut svg);
                let output = svg.finalize();
                return Ok(Some(GraphBlock {
                    input: input.into(),
                    output,
                }));
            }
            Err(err) => {
                let msg = format!("error parsing graph block: {}", err);
                return Err(msg);
            }
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
    use super::*;
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};

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
            let events = crate::parse_markdown(document)
                .into_iter()
                .map(|ee| format.transform_extended_event(ee))
                .flatten();
            let output = format.render(events);

            assert_eq!(expected, output);
        }
    }
}
