pub mod backend;
pub mod config;
pub mod frontend;
pub mod lexer;
pub mod libc;
pub mod ms;
pub mod native;
pub mod registries;
pub mod resolver;
pub mod scope;
pub mod utils;

pub use backend::compile::compile_binary;
