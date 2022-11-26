use std::fs;
use md::{markdown_to_html, StandaloneOptions, YamdrOptions};

fn main() {
    let md = fs::read_to_string("test.md").expect("failed to read markdown file");
    let options = YamdrOptions {
        standalone: Some(StandaloneOptions {}),
    };
    let (_, html) = markdown_to_html(options, &md);
    println!("{}", html);
}

