use clap::Parser;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::{thread, time::Duration};
use figlet_rs::{FIGlet, Toilet};

#[derive(Parser)]
#[command(name = "docraly")]
#[command(about = "Automatic API documentation from code")]
struct Args {
    file: String,
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

    let pb = ProgressBar::new_spinner();

    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );

    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message(format!("Analyzing {}", args.file));

    thread::sleep(Duration::from_secs(2));

    pb.finish_with_message("Analysis complete".green().to_string());
}