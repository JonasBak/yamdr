use crate::{CustomBlock, CustomBlockHeader, CustomBlockReader, Error, Format, Result};
use plotters::prelude::*;
use pulldown_cmark::{CodeBlockKind, Event, Tag};
use serde::{Deserialize, Serialize};

static COLORS: &[RGBColor] = &[RED, GREEN, BLUE, YELLOW, MAGENTA, CYAN, BLACK];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlottersBlock {
    LineChart {
        title: String,
        range_x: Option<(f32, f32)>,
        range_y: Option<(f32, f32)>,
        data: Vec<Vec<(f32, f32)>>,
    },
}

pub struct PlottersBlockReader {}

impl PlottersBlockReader {
    pub fn initial_state() -> Self {
        PlottersBlockReader {}
    }
}

impl CustomBlockReader for PlottersBlockReader {
    fn can_read_block(&self, header: &CustomBlockHeader) -> bool {
        header.t == "Plotters"
    }

    fn read_block(
        &mut self,
        header: &CustomBlockHeader,
        input: &str,
    ) -> Result<Option<Box<dyn CustomBlock>>> {
        if header.t != "Plotters" {
            todo!("unsupported block type")
        }
        let data = serde_yaml::from_str::<PlottersBlock>(input).map_err(|e| {
            Error::CustomBlockRead(format!("failed to parse block: {}", e))
        })?;
        Ok(Some(Box::new(data)))
    }
}

impl CustomBlock for PlottersBlock {
    fn to_events(&self, format: Format) -> Vec<Event<'static>> {
        match (self, format) {
            (_, Format::Md) => {
                let props: pulldown_cmark::CowStr =
                    serde_json::to_string(&CustomBlockHeader::empty("Plotters".into()))
                        .unwrap()
                        .into();
                let mut events = vec![Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(
                    props.clone(),
                )))];
                let body = serde_yaml::to_string(self).unwrap();
                events.push(Event::Text(body.into()));
                events.push(Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(props))));

                events
            }
            (
                PlottersBlock::LineChart {
                    title,
                    range_x,
                    range_y,
                    data,
                },
                Format::Html,
            ) => {
                let mut svg = String::new();
                {
                    let root = SVGBackend::with_string(&mut svg, (600, 400)).into_drawing_area();

                    let range_x = range_x.unwrap_or_else(|| {
                        (
                            0.0,
                            data.iter()
                                .flatten()
                                .map(|(x, _y)| x)
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .copied()
                                .unwrap_or(0.0),
                        )
                    });
                    let range_y = range_y.unwrap_or_else(|| {
                        (
                            0.0,
                            data.iter()
                                .flatten()
                                .map(|(_x, y)| y)
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .copied()
                                .unwrap_or(0.0),
                        )
                    });

                    let mut chart = ChartBuilder::on(&root)
                        .caption(title, ("sans-serif", 40).into_font())
                        .margin(10)
                        .set_left_and_bottom_label_area_size(20)
                        .build_cartesian_2d(range_x.0..range_x.1, range_y.0..range_y.1)
                        .unwrap();

                    chart
                        .configure_mesh()
                        .x_labels(5)
                        .y_labels(5)
                        .draw()
                        .unwrap();

                    for (i, points) in data.iter().enumerate() {
                        let color = COLORS[i % COLORS.len()];
                        chart
                            .draw_series(LineSeries::new(points.clone(), &color))
                            .unwrap();
                    }
                }
                vec![Event::Html(svg.into())]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn render_markdown() {
        let documents = [(
            r#"```{"t":"Plotters"}
type: LineChart
title: test
range_x: [0, 4]
range_y: [0, 4]
data:
- [[0, 0], [1, 1], [2, 2], [3, 3], [4, 4]]
```

"#,
            r#"```{"t":"Plotters"}
type: LineChart
title: test
range_x:
- 0.0
- 4.0
range_y:
- 0.0
- 4.0
data:
- - - 0.0
    - 0.0
  - - 1.0
    - 1.0
  - - 2.0
    - 2.0
  - - 3.0
    - 3.0
  - - 4.0
    - 4.0
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
