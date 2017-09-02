#[macro_use]
extern crate log;
extern crate fern;
extern crate getopts;
extern crate bufstream;
use getopts::Options;
use std::env;
use std::io::{Write, BufRead};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::collections::HashMap;
use bufstream::{BufStream};


fn print_usage(program: &str, opts: Options) {
    print!("{}", opts.usage(&brief(&program)));
}

fn brief<ProgramName>(program: ProgramName) -> String
        where ProgramName: std::fmt::Display {
    return format!("Usage: {} -o PASSWD [-p PORT] [(-q|-v|--vv)]", program);
}

enum Command {
	Nick(String), // nickname
	User(User), // user, mode, realname
}

struct User {
	user: String,
	mode: String,
	realname: String,
}

impl User {
	fn new(user: String, mode: String, realname: String) -> Self {
		User {user: user, mode: mode, realname: realname}
	}
}

struct IrcServer {
	nicknames: HashMap<SocketAddr, String>, 
	users: HashMap<SocketAddr, User>,
}

impl IrcServer {
	fn new() -> Self {
		IrcServer { nicknames: HashMap::new(), users: HashMap::new() }
	}

	fn handle_client(&mut self, stream: TcpStream) {
		let peer_address = stream.peer_addr().unwrap();
		let mut stream = BufStream::new(stream);
		loop {
			let mut buffer = String::new();

			if let Err(e) = stream.read_line(&mut buffer) {
				error!("Stream Read Error: {}", e);
				continue;
			}

			if buffer.is_empty() { break; }

			match parse_message(buffer) {
				Ok(Command::Nick(nick)) => {
					self.handle_nick(&mut stream, peer_address, nick);
				}
				Ok(Command::User(user)) => {
					self.handle_user(&mut stream, peer_address, user);
				}
				Err(e) => {
					error!("Message Parsing Error: {}", e);
				}
			}
		}
	}

	fn handle_nick(&mut self, stream: &mut BufStream<TcpStream>,
		peer_address: SocketAddr,
		nick: String) {
		trace!("got NICK message\nnick: {}", nick);
		// if let Err(e) = write!(stream, "Hi, {}!\r\n", nick) {
		// 	error!("Stream Write Error: {}", e);
		// }
		// if let Err(e) = stream.flush() {
		// 	error!("Stream Flush Error: {}", e);
		// }
		self.nicknames.insert(peer_address, nick);
		if self.users.contains_key(&peer_address) {
			self.send_reply(stream, peer_address);
		}
	}

	fn handle_user(&mut self, stream: &mut BufStream<TcpStream>,
		peer_address: SocketAddr,
		user: User) {
		trace!("got USER message\nuser: {}\nmode: {}\nrealname: {}", user.user, user.mode, user.realname);
		// if let Err(e) = write!(stream, "Hi, {}!\r\n", user.realname) {
		// 	error!("Stream Write Error: {}", e);
		// }
		// if let Err(e) = stream.flush() {
		// 	error!("Stream Flush Error: {}", e);
		// }
		self.users.insert(peer_address, user);
		if self.nicknames.contains_key(&peer_address) {
			self.send_reply(stream, peer_address);
		}
	}

	fn send_reply(&self, stream: &mut BufStream<TcpStream>, peer_address: SocketAddr) {
		if let Err(e) = write!(stream, "Welcome to the Internet Relay Network {}!{}@{}\r\n",
			self.nicknames[&peer_address], self.users[&peer_address].user, peer_address) {
			error!("Stream Write Error: {}", e);
		}
		if let Err(e) = stream.flush() {
			error!("Stream Flush Error: {}", e);
		}
	}
}

fn parse_message(message: String) -> Result<Command, &'static str> {
	debug!("\n\nmessage: {}", message);
	let msg_parts: Vec<&str> = message.trim_right().split(' ').collect();

	let command;
	let mut param_index = 1;
	let num_param;

	if msg_parts[0].starts_with(':') {
		let prefix = msg_parts[0];
		command = msg_parts[1];
		param_index = 2;
		num_param = msg_parts.len() - 2;
		debug!("prefix: {}", prefix);
	} else {
		command = msg_parts[0];
		num_param = msg_parts.len() - 1;
		debug!("no prefix");
	}
	
	debug!("command: {}", command);
	debug!("msg has {} params", num_param);
	match command {
		"NICK" => {
			if num_param != 1 {
				return Err("NICK needs 1 parameter");
			} else {
				return Ok(Command::Nick(msg_parts[param_index].to_string()));
			}
		},
		"USER" => {
			if num_param != 4 {
				return Err("USER needs 4 parameters");
			} else {
				return Ok(Command::User(
					User::new(
						msg_parts[param_index].to_string(),
				 		msg_parts[param_index+1].to_string(),
				 		msg_parts[param_index+3].to_string())
					));
			}
		},
		_ => {return Err("unknown command");}
	}
}


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

    let mut this_irc_server = IrcServer::new();

    // now do some actual network programming
    let listener = TcpListener::bind(("127.0.0.1", portnum)).unwrap();
    match listener.accept() {
	    Ok((socket, addr)) => {
	    	trace!("new client: {:?}", addr);
	    	this_irc_server.handle_client(socket);
	    }
	    Err(e) => error!("couldn't get client: {:?}", e),
	}
}
