use axum::{Form, Router, extract::State, response::Redirect, routing::post};
use clap::Parser;
use notify::{Config, EventKind, RecommendedWatcher, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs::OpenOptions,
    io::{Error as IOErrror, Write as IOWrite},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tera::Tera;
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{info, warn};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Snippie {
    title: String,
    contents: String,
}

#[derive(Clone, Debug, Parser)]
struct Args {
    #[arg(short, long, help = "Directory where snippie .md files reside")]
    snippie: String,

    #[arg(short, long, help = "Output directory, ready to serve")]
    out: Option<String>,

    #[arg(
        short,
        long,
        default_value_t = false,
        help = "Delete output directory contents before writing new files"
    )]
    clear_output: bool,

    #[arg(short, default_value_t = 8192, help = "Port to listen on")]
    port: u16,

    #[arg(
        long,
        default_value_t = false,
        help = "Watch for file changes (not needed when 'dev' is enabled)"
    )]
    watch: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "Start a server and watch for file changes"
    )]
    serve: bool,
}

impl Args {
    fn get_out_dir_or_default(&self) -> PathBuf {
        let output_dir = self.out.clone().unwrap_or(String::from("./output"));
        PathBuf::from(output_dir)
    }
}

// fn write_assets(args: &Args) -> Result<(), IOErrror> {
//     let output_dir = args.get_out_dir_or_default();
//     let prism_css = include_str!("prism.css");
//     let prism_js = include_str!("prism.js");

//     std::fs::write(output_dir.join("prism.css"), prism_css)?;
//     std::fs::write(output_dir.join("prism.js"), prism_js)?;

//     Ok(())
// }

fn write_snippies(args: &Args, snippies: &Vec<Snippie>) -> Result<(), IOErrror> {
    let output_dir = args.get_out_dir_or_default();
    let snippie_sub_dir = output_dir.join("snippies");

    if !snippie_sub_dir.exists() {
        std::fs::create_dir_all(&snippie_sub_dir)?;
    } else if args.clear_output {
        info!("Clearing existing output directory");
        std::fs::remove_dir_all(&output_dir)?;
        std::fs::create_dir_all(&snippie_sub_dir)?;
    }

    for snippie in snippies {
        let snippie_path = snippie_sub_dir.join(format!("{}.html", snippie.title));

        std::fs::write(snippie_path, &snippie.contents)?;
    }

    Ok(())
}

fn render_snippies(args: &Args, tera: &Tera) -> Result<Vec<Snippie>, IOErrror> {
    let files = std::fs::read_dir(&args.snippie)?;
    let mut snippies = vec![];
    let mut tera_context = tera::Context::new();
    tera_context.insert("title", "Snippie");

    for file in files.flatten() {
        let file_path = file.path();

        if file_path.is_dir() {
            continue;
        }

        let file_name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(std::string::ToString::to_string);

        if let Some(title) = file_name {
            let snippie_file_contents = std::fs::read_to_string(file_path)?;
            let snippie_rendered = markdown::to_html(&snippie_file_contents);
            tera_context.insert("content", &snippie_rendered);

            let snippie_content = tera.render("snippie.html", &tera_context).unwrap();

            snippies.push(Snippie {
                title,
                contents: snippie_content,
            });
        }
    }

    Ok(snippies)
}

fn copy_static_files(args: &Args) -> Result<(), IOErrror> {
    let output_dir = args.get_out_dir_or_default();
    let static_dir = output_dir.join("static");

    if !static_dir.exists() {
        std::fs::create_dir_all(&static_dir)?;
    } else {
        info!("Clearing existing output directory");
        std::fs::remove_dir_all(&static_dir)?;
        std::fs::create_dir_all(&static_dir)?;
    }

    std::fs::copy("frontend/static/app.css", static_dir.join("app.css"))?;
    std::fs::copy("frontend/static/prism.css", static_dir.join("prism.css"))?;
    std::fs::copy("frontend/static/prism.js", static_dir.join("prism.js"))?;

    Ok(())
}

fn create_snippies(args: &Args) -> Result<(), IOErrror> {
    info!("Creating snippies");

    let output_dir = args.get_out_dir_or_default();

    let mut tera = Tera::new("frontend/templates/*.html").unwrap();
    tera.autoescape_on(vec![]);

    let snippies = render_snippies(args, &tera)?;
    write_snippies(args, &snippies)?;

    let mut tera_context = tera::Context::new();
    tera_context.insert("title", "Snippie");
    tera_context.insert("snippies", &snippies);
    let rendered = tera.render("index.html", &tera_context).unwrap();

    std::fs::write(output_dir.join("index.html"), rendered)?;

    copy_static_files(args)?;

    info!("Snippies created successfully");
    Ok(())
}

fn get_current_timestamp() -> u64 {
    let current_time = SystemTime::now();
    current_time
        .duration_since(UNIX_EPOCH)
        .expect("Weird time error")
        .as_secs()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let file_watch_args = args.clone();
    let output_dir = args.get_out_dir_or_default();
    let mut error_path = output_dir.clone();
    error_path.push("error.html");

    create_snippies(&args)?;

    if args.serve || args.watch {
        let (file_watch_tx, mut file_watch_rx) =
            tokio::sync::watch::channel(get_current_timestamp());

        let file_watch_handle = tokio::spawn(async move {
            loop {
                let updated = file_watch_rx.changed().await;

                match updated {
                    Ok(()) => {
                        let update_time = *file_watch_rx.borrow_and_update();

                        if update_time == 0 {
                            info!("File watch thread exiting. Reason: Exit code received");
                            break;
                        }

                        if let Err(error) = create_snippies(&file_watch_args) {
                            warn!("Could not create snippies. Error: {}", error);
                        }
                    }
                    Err(e) => {
                        info!("File watch thread exiting. Reason: {}", e);
                        break;
                    }
                }
            }

            info!("File watch build thread finished");
        });

        let mut file_watcher = RecommendedWatcher::new(
            move |e: Result<notify::Event, notify::Error>| match e {
                Ok(e) => match e.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        file_watch_tx.send(get_current_timestamp()).unwrap();
                    }
                    _ => info!("Ignored event of kind {:?}", e.kind),
                },
                Err(_) => file_watch_tx.send(0).unwrap(),
            },
            Config::default(),
        )?;

        file_watcher.watch(
            Path::new(&args.snippie),
            notify::RecursiveMode::NonRecursive,
        )?;

        if args.serve {
            let app = Router::new()
                .route_service("/", ServeFile::new(output_dir.join("index.html")))
                .route_service("/error", ServeFile::new(&error_path))
                .route("/new", post(new_snippie_route))
                .nest_service("/snippie", ServeDir::new(output_dir.join("snippies")))
                .nest_service("/static", ServeDir::new(output_dir.join("static")))
                .with_state(args.clone());

            let listener = TcpListener::bind(format!("0.0.0.0:{}", args.port)).await?;
            info!("Dev mode enabled. Listening on {}", args.port);
            axum::serve(listener, app).await?;
        } else if args.watch {
            info!("Listening for filesystem changes");
            file_watch_handle.await?;
        }
    }

    Ok(())
}

#[axum::debug_handler]
async fn new_snippie_route(State(state): State<Args>, Form(data): Form<Snippie>) -> Redirect {
    info!("Creating new snippie: {:?}", &data.title);

    let mut snippie_file_path = PathBuf::from(state.snippie);
    snippie_file_path.push(&data.title);
    snippie_file_path.add_extension("md");

    let new_snippie = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(snippie_file_path);

    match new_snippie {
        Ok(mut file) => {
            if let Err(write_error) = file.write_all(data.contents.as_bytes()) {
                warn!(
                    "Could not write to new Snippie file. Reason: {}",
                    write_error
                );
                Redirect::to("/error")
            } else {
                // Wait for snippies to be rebuilt, so we don't accidentally run into a 404 error
                tokio::time::sleep(Duration::from_millis(100)).await;

                Redirect::to("/")
            }
        }
        Err(error) => {
            warn!("Could not create Snippie. Reason: {}", error);
            Redirect::to("/error")
        }
    }
}
