#![forbid(unsafe_code)]

mod cli;
mod command_build;
mod command_build_leptos;
mod command_build_web;
mod command_coverage;
mod command_detect;
mod command_doctor;
mod command_export_web_json;
mod command_extract;
mod command_init;
mod command_pseudo;
mod command_scaffold;
mod command_sign;
mod command_validate;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let json = cli::args_request_json(&args);
    if let Err(err) = cli::run_args(args) {
        if json {
            match cli::render_error_json(&err) {
                Ok(payload) => eprintln!("{payload}"),
                Err(json_err) => eprintln!("{json_err}"),
            }
        } else {
            eprintln!("{err}");
        }
        std::process::exit(1);
    }
}
