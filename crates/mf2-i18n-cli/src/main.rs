#![forbid(unsafe_code)]

mod config;
mod catalog;
mod catalog_builder;
mod artifacts;
mod cli;
mod command_extract;
mod diagnostic;
mod error;
mod extract;
mod extract_pipeline;
mod model;
mod id_map;
mod lexer;
mod parser;
mod validator;
mod compiler;

fn main() {
    if let Err(err) = cli::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
