mod model;
mod parser;
mod manifest;
mod discovery;
mod scanner;
mod adapter;

use clap::{Parser, Subcommand};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::{fs, thread, time::Duration};
use figlet_rs::Toilet;
use adapter::AdapterRegistry;
use discovery::{discover_project, Language};
use scanner::scan_project_sources;

const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHOR: &str = env!("CARGO_PKG_AUTHORS");
const WEBSITE: &str = env!("CARGO_PKG_REPOSITORY");
const LICENSE: &str = env!("CARGO_PKG_LICENSE");

#[derive(Parser)]
#[command(name = "docraly")]
#[command(about = DESCRIPTION)]
#[command(author = AUTHOR)]
#[command(version = VERSION)]
struct Args {
    #[arg(short, long)]
    file: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a project directory to detect languages and frameworks
    Scan {
        /// Path to the project root directory
        #[arg(default_value = ".")]
        path: String,
    },
}

fn print_banner() {
    let font = Toilet::mono12().unwrap();
    let figure = font.convert("DOCRALY").unwrap().to_string();

    for line in figure.lines() {
        let width = line.chars().count();
        let mut line_output = String::new();

        for (index, character) in line.chars().enumerate() {
            let position = index as f32 / width.max(1) as f32;

            let colored_character = if position < 0.33 {
                character.to_string().green()
            } else if position < 0.66 {
                character.to_string().white()
            } else {
                character.to_string().green()
            };

            line_output.push_str(&colored_character.to_string());
        }

        println!("{}", line_output); 
    }

   
    println!();
    println!("About:{}", DESCRIPTION.white());
    println!("Author:{}", AUTHOR.white());
    println!("Website:{}", WEBSITE.white());
    println!("Version:{}", VERSION.white());
    println!("License:{}", LICENSE.white());
    println!();
}


fn main() {
    let args = Args::parse();

    print_banner();

    if let Some(Commands::Scan { path }) = &args.command {
        run_scan(path);
        return;
    }

    if let Some(file) = args.file {
        let file_path = std::path::Path::new(&file);
        let source = match fs::read_to_string(file_path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("{} {}", "Error:".red(), error);
                std::process::exit(1);
            }
        };

        let pb = ProgressBar::new_spinner();

        pb.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg}")
                .unwrap()
                .tick_strings(&[
                    "⠋", "⠙", "⠹", "⠸", "⠼",
                    "⠴", "⠦", "⠧", "⠇", "⠏",
                ]),
        );

        pb.enable_steady_tick(Duration::from_millis(100));

        // Stage 1
        pb.set_message("Scanning project...");
        thread::sleep(Duration::from_secs(1));

        // Stage 2
        pb.set_message("Parsing PHP...");
        let registry = AdapterRegistry::new();
        let adapter = registry
            .get(Language::Php)
            .expect("PHP adapter missing");

        let file_model = match adapter.parse_file(file_path, file_path, &source) {
            Ok(model) => model,
            Err(error) => {
                pb.finish_and_clear();
                eprintln!("{} {}", "Error:".red(), error);
                std::process::exit(1);
            }
        };
        thread::sleep(Duration::from_secs(1));

        // Stage 2.5
        pb.set_message("Analyzing AST...");
        thread::sleep(Duration::from_millis(500));

        // Stage 3
        pb.set_message("Analyzing routes...");
        thread::sleep(Duration::from_secs(1));

        // Stage 4
        pb.set_message(format!("Analyzing {}", file));
        thread::sleep(Duration::from_secs(1));

        pb.finish_with_message("✓ Analysis complete".green().to_string());

        if let Some(namespace) = &file_model.namespace {
            println!("Namespace: {}", namespace.cyan());
        }

        if !file_model.imports.is_empty() {
            println!("\n=== Imports ===");
            for import in &file_model.imports {
                match &import.alias {
                    Some(alias) => println!("  use {} as {};", import.path, alias),
                    None => println!("  use {};", import.path),
                }
            }
        }

        println!();
        println!("{}", "=== Top-level Functions ===".yellow());
        if file_model.functions.is_empty() {
            println!("  (none)");
        } else {
            for function in &file_model.functions {
                println!("{:#?}", function);
            }
        }

        println!("{}", "\n=== Classes & Methods ===".yellow());
        if file_model.classes.is_empty() {
            println!("  (none)");
        } else {
            for class in &file_model.classes {
                println!("{:#?}", class);
            }
        }
    } else {
        println!("{}", "Run `docraly scan [path]` to scan a project, or `docraly --help` for options.".cyan());
    }
}

fn run_scan(path: &str) {
    let root = std::path::Path::new(path);

    if !root.exists() {
        eprintln!("{}", format!("Error: Path does not exist: {}", path).red());
        std::process::exit(1);
    }

    let project = discover_project(root);

    println!("{}", "Project detected:".green().bold());
    println!("Root: {}", project.root.display());
    println!();

    println!("{}", "Languages:".cyan().bold());
    if project.languages.is_empty() {
        println!("  (none detected)");
    } else {
        for language in &project.languages {
            println!("  - {:?}", language);
        }
    }

    println!();
    println!("{}", "Frameworks:".cyan().bold());
    if project.frameworks.is_empty() {
        println!("  (none detected)");
    } else {
        for framework in &project.frameworks {
            println!("  - {:?}", framework);
        }
    }

    let source_files = scan_project_sources(&project);

    println!();
    println!(
        "{}",
        format!("Source files ({} found):", source_files.len())
            .cyan()
            .bold()
    );
    if source_files.is_empty() {
        println!("  (none found)");
    } else {
        for file in &source_files {
            println!("  - {}", file.relative_path.display());
        }
    }

    let registry = AdapterRegistry::new();
    let project_model = registry.parse_project(&source_files);

    if !project_model.files.is_empty() {
        let total_classes: usize = project_model.files.iter().map(|f| f.classes.len()).sum();
        let total_methods: usize = project_model
            .files
            .iter()
            .flat_map(|f| &f.classes)
            .map(|c| c.methods.len())
            .sum();
        let total_functions: usize = project_model.files.iter().map(|f| f.functions.len()).sum();
        println!();
        println!(
            "{}",
            format!(
                "Project parsed: {} files, {} classes ({} methods), {} standalone functions",
                project_model.files.len(),
                total_classes,
                total_methods,
                total_functions
            )
            .green()
            .bold()
        );
    }
}