#![forbid(unsafe_code)]

mod cli;
mod command_build;
mod command_coverage;
mod command_extract;
mod command_pseudo;
mod command_sign;
mod command_validate;

fn main() {
    if let Err(err) = cli::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
