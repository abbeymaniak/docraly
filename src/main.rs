mod model;
mod parser;

use clap::Parser;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::{fs, thread, time::Duration};
use figlet_rs::Toilet;
use parser::{find_classes, find_functions, parse_php};


#[derive(Parser)]
#[command(name = "docraly")]
#[command(about = "Automatic API documentation from code")]
struct Args {
    file: Option<String>,
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
    println!("{}", "Automatic API documentation from code".white());
    println!();
}


fn main() {
    let args = Args::parse();

    print_banner();

    if let Some(file) = args.file {

    let source = match fs::read_to_string(&file) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{} {}", "Error:".red(), error);
            return;
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

         let tree = match parser::parse_php(&source) {
        Ok(tree) => tree,
        Err(error) => {
            pb.finish_and_clear();
            eprintln!("{} {}", "Error:".red(), error);
            return;
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

        //  println!();
        //  println!("Root node: {}", tree.root_node().kind());

        //  println!();
        // print_ast(tree.root_node(), &source, 0);


        // println!();
        // println!("{}","\n=== Analyzing Functions ===".yellow());
        // find_functions(tree.root_node(), &source);

        let functions = find_functions(tree.root_node(), &source);


        println!("=== AST ===");
// print_ast(tree.root_node(), &source);

        println!();
        println!("{}","\n=== Analyzing Functions ===".yellow());
        for function in &functions {
            println!("{:#?}", function);
        }

        let classes = find_classes(tree.root_node(), &source);

println!("=== Analyzing Classes ===");

for class in &classes {
    println!("{:#?}", class);
}

       
        }
}