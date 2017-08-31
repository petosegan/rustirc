extern crate log;
extern crate fern;
extern crate getopts;
use getopts::Options;
use std::env;
use std::io::Write;

fn print_usage(program: &str, opts: Options) {
    print!("{}", opts.usage(&brief(&program)));
}

fn brief<ProgramName>(program: ProgramName) -> String
        where ProgramName: std::fmt::Display {
    return format!("Usage: {} -o PASSWD [-p PORT] [(-q|-v|--vv)]", program);
}

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
	
    println!("Hello, world!");
}
