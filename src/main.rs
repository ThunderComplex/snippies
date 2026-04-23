use axum::{
    Form, Router,
    body::Body,
    extract::{Request, State},
    http::{Response, header},
    middleware::{self, Next},
    response::{IntoResponse, Redirect},
    routing::post,
};
use clap::Parser;
use notify::{Config, EventKind, RecommendedWatcher, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::Write,
    io::Error as IOErrror,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
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

fn write_html_files(index: String, snippies: Vec<Snippie>, args: &Args) -> Result<(), IOErrror> {
    let output_dir = args.get_out_dir_or_default();

    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir)?;
    } else if args.clear_output {
        info!("Clearing existing output directory");
        std::fs::remove_dir_all(&output_dir)?;
        std::fs::create_dir_all(&output_dir)?;
    }

    let index_path = output_dir.join("index.html");

    std::fs::write(index_path, index)?;

    for snippie in snippies {
        let snippie_path = output_dir.join(format!("{}.html", snippie.title));

        std::fs::write(snippie_path, snippie.contents)?;
    }

    Ok(())
}

fn write_assets(args: &Args) -> Result<(), IOErrror> {
    let output_dir = args.get_out_dir_or_default();
    let prism_css = include_str!("prism.css");
    let prism_js = include_str!("prism.js");

    std::fs::write(output_dir.join("prism.css"), prism_css)?;
    std::fs::write(output_dir.join("prism.js"), prism_js)?;

    Ok(())
}

fn render_snippies_in_path(path: &Path) -> Result<Vec<Snippie>, IOErrror> {
    let files = std::fs::read_dir(path)?;
    let mut snippies = vec![];

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
            let snippie_template = include_str!("template.html");
            let snippie_content = snippie_template
                .replace(r"{{$_TITLE}}", &title)
                .replace(r"{{$_CONTENT}}", &snippie_rendered);

            snippies.push(Snippie {
                title,
                contents: snippie_content,
            });
        }
    }

    Ok(snippies)
}

fn create_snippies(args: &Args) -> Result<(), IOErrror> {
    info!("Creating snippies");

    let index = include_str!("index.html");

    let snippies = render_snippies_in_path(Path::new(&args.snippie))?;

    let snippie_links = snippies.iter().fold(String::new(), |mut acc, s| {
        let _ = write!(acc, "<li><a href='{}.html'>{}</a></li>", s.title, s.title);
        acc
    });

    let snippie_index = index.replace(r"{{$_CONTENT}}", &snippie_links);

    let _ = write_html_files(snippie_index, snippies, &args);
    let _ = write_assets(&args);

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
                .route("/new", post(new_snippie_route))
                .fallback_service(ServeDir::new(&output_dir))
                .layer(middleware::from_fn(display_error_middleware))
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

async fn display_error_middleware(request: Request, next: Next) -> impl IntoResponse {
    dbg!(&request);
    let next_resp = next.run(request).await;
    let (mut parts, body) = next_resp.into_parts();

    if let Some(error_header) = parts.headers.get("SNIPPIE_ERROR") {
        info!("Displaying snippie error");
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let contents: Vec<u8> = dbg!(bytes).into_iter().collect();
        let contents_str = String::from_utf8(dbg!(contents)).unwrap();

        let replaced = contents_str.replace(
            r"{{$_ERROR}}",
            error_header
                .to_str()
                .unwrap_or("Snippie error but could not decode header"),
        );

        parts.headers.remove(header::CONTENT_LENGTH);
        parts
            .headers
            .insert(header::CONTENT_LENGTH, replaced.len().into());

        return Response::from_parts(parts, Body::from(replaced));
    }

    Response::from_parts(parts, body)
}

#[axum::debug_handler]
async fn new_snippie_route(
    State(state): State<Args>,
    Form(data): Form<Snippie>,
) -> impl IntoResponse {
    info!("Creating new snippie: {:?}", &data.title);

    let mut snippie_file_path = PathBuf::from(state.snippie);
    snippie_file_path.push(&data.title);
    snippie_file_path.add_extension("md");

    let mut error_header = [("SNIPPIE_ERROR", "DEEZ NUTZ".into())];

    // TODO: Should probably improve this in the future and make the error visible on the frontend
    if let Err(error) = std::fs::write(snippie_file_path, data.contents) {
        warn!("Could not create Snippie. Reason: {}", error);
        error_header = [("SNIPPIE_ERROR", error.to_string())];
    };

    (error_header, Redirect::to("/"))
}
