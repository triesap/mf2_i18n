#![forbid(unsafe_code)]

mod artifacts;
mod catalog;
mod catalog_builder;
mod catalog_reader;
mod cli;
mod command_build;
mod command_coverage;
mod command_extract;
mod command_pseudo;
mod command_sign;
mod command_validate;
mod compiler;
mod config;
mod diagnostic;
mod error;
mod extract;
mod extract_pipeline;
mod id_map;
mod lexer;
mod locale_sources;
mod manifest;
mod mf2_source;
mod micro_locales;
mod model;
mod pack_encode;
mod parser;
mod validator;

fn main() {
    if let Err(err) = cli::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
