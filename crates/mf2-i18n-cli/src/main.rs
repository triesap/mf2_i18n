#![forbid(unsafe_code)]

mod config;
mod catalog;
mod catalog_builder;
mod catalog_reader;
mod artifacts;
mod cli;
mod command_extract;
mod command_build;
mod command_validate;
mod command_sign;
mod command_pseudo;
mod command_coverage;
mod diagnostic;
mod error;
mod extract;
mod extract_pipeline;
mod mf2_source;
mod manifest;
mod model;
mod micro_locales;
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
