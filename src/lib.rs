#[macro_use]
extern crate log;
extern crate bufstream;

mod parser;
mod server;
mod connection;

pub use server::IrcServer;