use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{routing::get, Router};
use clap::{Parser, Subcommand};
use futures::stream::{self, Stream};
use md::{markdown_to_html, StandaloneOptions, YamdrOptions};
use std::fs;
use tokio_stream::StreamExt as _;

#[derive(Parser, Debug)]
#[command(name = "yamdr", about = "TODO about", long_about = None)]
struct Args {
    /// Markdown file to parse
    #[arg(short, long)]
    file: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Render file to html
    #[command(arg_required_else_help = true)]
    Render {
        /// output file or "-" for stdout
        output: String,
    },
    /// Serve rendered file
    Serve {
        #[arg(long, short, default_value_t = false)]
        watch: bool,
    },
}

static HOT_RELOAD_JS: &str = r#"
<script>
const eventSource = new EventSource("/watch");
eventSource.onmessage = function(e) {
  const htmlElement = document.querySelector("html");
  if (e.data) {
    console.log("Reloading...");
    htmlElement.innerHTML = e.data;
  }
};
</script>
"#;

fn get_modified(file: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(file)
        .ok()
        .map(|m| m.modified().ok())
        .flatten()
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let mut options = YamdrOptions {
        standalone: Some(StandaloneOptions {}),
        additional_head: None,
        additional_body: None,
    };

    match args.command {
        Commands::Render { output } => {
            let md = fs::read_to_string(&args.file)
                .expect(&format!("failed to read file {}", args.file));

            let (_, html) = markdown_to_html(&options, &md);
            if output == "-" {
                println!("{html}");
            } else {
                fs::write(&output, html).expect(&format!("failed to write output to {output}"));
            }
        }
        Commands::Serve { watch } => {
            if watch {
                options.additional_head = Some(HOT_RELOAD_JS.to_string());
            }
            let app = Router::new()
                .route("/", {
                    let options = options.clone();
                    let file = args.file.clone();
                    get(move || async move {
                        let md = fs::read_to_string(&file)
                            .expect(&format!("failed to read file {}", &file));

                        let (_, html) = markdown_to_html(&options, &md);

                        axum::response::Html(html)
                    })
                })
                .route("/watch", {
                    let options = options.clone();
                    let file = args.file.clone();
                    get(move || async move {
                        let mut last = get_modified(&file);
                        let stream = stream::repeat_with(move || {
                            if !watch {
                                return None;
                            }
                            let new = get_modified(&file);
                            if match (last, new) {
                                (Some(last), Some(new)) if new > last => true,
                                (None, Some(_)) => true,
                                _ => false,
                            } {
                                last = new;
                                let md = fs::read_to_string(&file)
                                    .expect(&format!("failed to read file {}", &file));

                                let (_, html) = markdown_to_html(&options, &md);
                                Some(html)
                            } else {
                                None
                            }
                        })
                        .throttle(std::time::Duration::from_secs(1))
                        .filter_map(|a| a)
                        .map(|a| Ok::<Event, std::convert::Infallible>(Event::default().data(a)));

                        Sse::new(stream).keep_alive(KeepAlive::default())
                    })
                });
            axum::Server::bind(&"127.0.0.1:3000".parse().unwrap())
                .serve(app.into_make_service())
                .await
                .unwrap();
        }
    }
}
