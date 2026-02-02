#![forbid(unsafe_code)]

mod config;
mod catalog;
mod catalog_builder;
mod catalog_reader;
mod artifacts;
mod cli;
mod command_extract;
mod command_validate;
mod diagnostic;
mod error;
mod extract;
mod extract_pipeline;
mod mf2_source;
mod model;
mod pack_encode;
mod id_map;
mod lexer;
mod locale_sources;
mod parser;
mod validator;
mod compiler;

fn main() {
    if let Err(err) = cli::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
