#[macro_use]
extern crate log;
extern crate bufstream;

mod parser;
mod server;

pub use server::IrcServer;