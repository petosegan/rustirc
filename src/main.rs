#[macro_use]
extern crate log;
extern crate fern;
extern crate getopts;
extern crate rustirc;
use getopts::Options;
use std::env;
use std::io::{Write};

use rustirc::IrcServer;

fn print_usage(program: &str, opts: Options) {
    print!("{}", opts.usage(&brief(&program)));
}

fn brief<ProgramName>(program: ProgramName) -> String
        where ProgramName: std::fmt::Display {
    return format!("Usage: {} -o PASSWD [-p PORT] [(-q|-v|--vv)]", program);
}

/* 
New streams will spawn threads (Connection) to own them.
Connections will own a receiver channel.
All connections share references to:
	nicknames
	users
	phonebook
The annoying bit: each Connection has to read and write
to its stream. It has to monitor both the stream and the
channel. Not sure how to do this.
*/

#[allow(unused_must_use)]
fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

	let mut opts = getopts::Options::new();
	opts.reqopt("o", "", "operator password", "PASSWD");
	opts.optopt("p", "port", "the port on which the server will listen", "PORT");
	opts.optflag("q", "quiet", "quiet mode. No log messages will be printed");
	opts.optflag("v", "", "print DEBUG messages");
	opts.optflag("", "vv", "print TRACE messages");
	opts.optflag("h", "help", "print this help message");

	let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => {let message = format!("{}\n{}\n",
                                  f.to_string(),
                                  opts.usage(&brief(&args[0])));
            if let Err(err) = write!(std::io::stderr(), "{}", message) {
                panic!("Failed to write to standard error: {}\n\
                       Error encountered while trying to log the \
                       following message: \"{}\"",
                       err,
                       message);
            }
            std::process::exit(1);
        }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }
    let mut logging_level = log::LogLevelFilter::Info;
    if matches.opt_present("v") {
        logging_level = log::LogLevelFilter::Debug;
    }
    if matches.opt_present("vv") {
        logging_level = log::LogLevelFilter::Trace;
    }
    if matches.opt_present("q") {
        logging_level = log::LogLevelFilter::Off;
    }

    let port = match matches.opt_str("p") {
        Some(s) => s.parse::<u16>(),
        None => "6667".parse::<u16>(),
    };
    let portnum;
    match port {
    	Ok(p) => {portnum = p;},
    	Err(_) => {panic!("Invalid port");}
    }
    let op_passwd = match matches.opt_str("o") {
        Some(s) => s,
        None => "swordfish".to_string(),
    };

    fern::Dispatch::new()
	    .format(|out, message, record| {
	        out.finish(format_args!(
	            "[{}] {}",
	            record.level(),
	            message
	        ))
	    })
	    .level(logging_level)
	    .chain(std::io::stdout())
	    .apply();
	
    trace!("\nOperator Password: {}\nPort: {}", op_passwd, portnum);
    info!("INFO is printing.");
    debug!("DEBUG is printing.");
    trace!("TRACE is printing.");

    let mut this_irc_server = IrcServer::new(portnum);
    this_irc_server.run();
}
