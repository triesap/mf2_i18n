#![forbid(unsafe_code)]

pub mod artifacts;
pub mod catalog;
pub mod catalog_builder;
pub mod catalog_reader;
pub mod compiler;
pub mod config;
pub mod diagnostic;
pub mod error;
pub mod extract;
pub mod extract_pipeline;
pub mod id_map;
pub mod lexer;
pub mod locale_sources;
pub mod manifest;
pub mod mf2_source;
pub mod micro_locales;
pub mod model;
pub mod pack_encode;
pub mod parser;
pub mod project;
pub mod validator;

pub use crate::config::{ProjectConfig, load_project_config, load_project_config_or_default};
pub use crate::error::BuildIoError;
pub use crate::project::{ProjectError, ProjectLayout, resolve_config_relative_path};
