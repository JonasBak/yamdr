use pulldown_cmark::{
    escape::escape_html, html, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag,
};

use crate::ExtendedEvent;

fn start_tag(tag: &Tag, parent_tags: &Vec<Tag>, event_count: u64) -> String {
    match tag {
        Tag::Heading(HeadingLevel::H1, _, _) => "# ".into(),
        Tag::Heading(HeadingLevel::H2, _, _) => "## ".into(),
        Tag::Heading(HeadingLevel::H3, _, _) => "### ".into(),
        Tag::Heading(HeadingLevel::H4, _, _) => "#### ".into(),
        Tag::Heading(HeadingLevel::H5, _, _) => "##### ".into(),
        Tag::Heading(HeadingLevel::H6, _, _) => "###### ".into(),
        Tag::CodeBlock(CodeBlockKind::Fenced(props)) => format!("```{}\n", props),
        Tag::TableCell => "| ".into(),
        Tag::Item => {
            let level = parent_tags
                .iter()
                .filter(|tag| matches!(tag, Tag::Item))
                .count();
            let indent = (if event_count as usize + level == 1 {
                ""
            } else {
                "\n"
            })
            .to_string()
                + ("  ".repeat(level)).as_str();
            match parent_tags.last() {
                Some(Tag::List(Some(i))) => format!("{}{}. ", indent, i + event_count - 1),
                Some(Tag::List(None)) => format!("{}- ", indent),
                _ => unreachable!(),
            }
        }
        Tag::Link(_, _, _) => "[".into(),
        _ => "".into(),
    }
}

fn end_tag(tag: &Tag, parent_tags: &Vec<Tag>, event_count: u64) -> String {
    match tag {
        Tag::Heading(_, _, _) => "\n\n".into(),
        Tag::CodeBlock(_) => "```\n\n".into(),
        Tag::Table(_) => "\n".into(),
        Tag::TableHead => {
            let Some(Tag::Table(table)) = parent_tags.last() else {
                panic!();
            };
            "|\n".to_string()
                + table
                    .iter()
                    .map(|_| "|---".to_string())
                    .collect::<String>()
                    .as_str()
                + "|\n"
        }
        Tag::TableRow => "|\n".into(),
        Tag::TableCell => " ".into(),
        Tag::List(_) if !matches!(parent_tags.last(), Some(Tag::Item)) => "\n\n".into(),
        Tag::Link(_, dest, _) => format!("]({})", dest),
        Tag::Paragraph => "\n\n".into(),
        _ => "".into(),
    }
}

pub fn render<'a>(mut events: impl Iterator<Item = Event<'a>>) -> String {
    let mut md_output = String::new();

    let mut tag_stack = Vec::new();
    let mut event_count = Vec::new();

    while let Some(event) = events.next() {
        event_count.last_mut().map(|n| *n += 1);
        match event {
            Event::Start(tag) => {
                md_output +=
                    start_tag(&tag, &tag_stack, event_count.last().copied().unwrap_or(0)).as_str();
                tag_stack.push(tag);
                event_count.push(0);
            }
            Event::End(tag) => {
                tag_stack.pop();
                event_count.pop();
                md_output +=
                    end_tag(&tag, &tag_stack, event_count.last().copied().unwrap_or(0)).as_str();
            }
            Event::Text(text) => {
                md_output += &text;
            }
            Event::Code(text) => {
                md_output += "`";
                md_output += &text;
                md_output += "`";
            }
            Event::SoftBreak => {
                md_output += "\n";
            }
            Event::HardBreak => {
                md_output += "\n\n";
            }
            Event::Rule => {
                md_output += "-----\n";
            }
            Event::Html(html) => {
                md_output += &html;
            }
            _ => todo!("{:?}", event),
        }
    }

    md_output
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};

    #[test]
    fn simple_tags() {
        let tags = [
            r#"# Heading 1

"#,
            r#"## Heading 2

"#,
            r#"### Heading 3

"#,
            r#"#### Heading 4

"#,
            r#"##### Heading 5

"#,
            r#"###### Heading 6

"#,
            r#"```
code block
```

"#,
            r#"```props
code block with props
```

"#,
            r#"| head A | head B | head C |
|---|---|---|
| 1 | 2 | 3 |
| 4 | 5 | 6 |
| 7 | 8 | 9 |

"#,
            r#"Text with inline `code`!

"#,
            r#"<div>Some html</div>"#,
            r#"- Item 1
- Item 2
- Item 3

"#,
            r#"1. Item 1
2. Item 2
3. Item 3

"#,
            r#"5. Item 1
6. Item 2
7. Item 3

"#,
            r#"[link](http://example.com)

"#,
            r#"- Item 1
  - Item 2
    - Item 3
  - Item 4
- Item 5
  - Item 6

"#,
        ];
        let md_options = Options::all();
        for tag in tags {
            let parser = Parser::new_ext(tag, md_options);
            let output = render(parser);
            assert_eq!(tag, output);
        }
    }

    #[test]
    fn documents() {
        let documents = [r#"# Heading

First paragraph. Some `inline code`. [a link](http://example.com).
Should be same paragraph.

New paragraph.

## Unordered list

- a
- b
- c

## Ordered list

1. a
2. b
3. c

### Starting at something other than 1

3. c
4. d
5. e

"#];
        let md_options = Options::all();
        for document in documents {
            let parser = Parser::new_ext(document, md_options);
            let output = render(parser);
            assert_eq!(document, output);
        }
    }
}
