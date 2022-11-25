use std::fs;
use yamdr::{markdown_to_html, YamdrOptions};

fn main() {
    let md = fs::read_to_string("test.md").expect("failed to read markdown file");
    let options = YamdrOptions { standalone: None };
    let (_, html) = markdown_to_html(options, &md);
    println!("{}", html);
}
