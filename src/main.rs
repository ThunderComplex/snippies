use clap::Parser;
use std::{io::Error, path::Path};

#[derive(Clone, Debug)]
struct Snippie {
    title: String,
    contents: String,
}

#[derive(Clone, Debug, Parser)]
struct Args {
    #[arg(short, long)]
    snippie_dir: String,

    #[arg(short, long)]
    out_dir: Option<String>,

    #[arg(short, long, default_value_t = false)]
    clear_output_dir: bool,
}

fn write_html_files(index: String, snippies: Vec<Snippie>, args: &Args) -> Result<(), Error> {
    let output_dir = args.out_dir.clone().unwrap_or(String::from("./output"));
    let output_dir = Path::new(&output_dir);

    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)?;
    } else if args.clear_output_dir {
        println!("[INFO] Clearing existing output directory");
        std::fs::remove_dir_all(output_dir)?;
        std::fs::create_dir_all(output_dir)?;
    }

    let index_path = output_dir.join("index.html");

    std::fs::write(index_path, index)?;

    for snippie in snippies {
        let snippie_path = output_dir.join(format!("{}.html", snippie.title));

        std::fs::write(snippie_path, snippie.contents)?;
    }

    Ok(())
}

fn write_assets(args: &Args) -> Result<(), Error> {
    let output_dir = args.out_dir.clone().unwrap_or(String::from("./output"));
    let output_dir = Path::new(&output_dir);
    let prism_css = include_str!("prism.css");
    let prism_js = include_str!("prism.js");

    std::fs::write(output_dir.join("prism.css"), prism_css)?;
    std::fs::write(output_dir.join("prism.js"), prism_js)?;

    Ok(())
}

fn render_snippies_in_path(path: &Path) -> Result<Vec<Snippie>, Error> {
    let files = std::fs::read_dir(path)?;
    let mut snippies = vec![];

    for file in files {
        if let Ok(file_entry) = file {
            let file_path = file_entry.path();

            if file_path.is_dir() {
                continue;
            }

            let file_name = file_path
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());

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
    }

    Ok(snippies)
}

fn main() -> Result<(), Error> {
    let args = Args::parse();
    let index = include_str!("index.html");

    let snippies = render_snippies_in_path(Path::new(&args.snippie_dir))?;

    let snippie_links = snippies
        .iter()
        .map(|s| format!("<li><a href='{}.html'>{}</a></li>", s.title, s.title))
        .collect::<Vec<String>>()
        .join("");

    let snippie_index = index.replace(r"{{$_CONTENT}}", &snippie_links);

    let _ = write_html_files(snippie_index, snippies, &args);
    let _ = write_assets(&args);

    Ok(())
}
