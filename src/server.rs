use std::net::{TcpListener, TcpStream, SocketAddr, IpAddr, Ipv4Addr};
use std::collections::HashMap;
use bufstream::{BufStream};
use std::io::{Write, BufRead};

// mod parser;
use parser::{Command, User, parse_message};

pub struct IrcServer {
	nicknames: HashMap<SocketAddr, String>, 
	users: HashMap<SocketAddr, User>,
	local_address: SocketAddr,
	peer_address: SocketAddr,
	portnum: u16,
}

impl IrcServer {
	pub fn new(portnum: u16) -> Self {
		IrcServer { nicknames: HashMap::new(),
			users: HashMap::new(),
			local_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
			peer_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
			portnum: portnum}
	}

	pub fn run(&mut self) {
		let listener = TcpListener::bind(("127.0.0.1", self.portnum)).unwrap();
	    for socket in listener.incoming() {
	    	match socket {
	    		Ok(stream) => {self.handle_client(stream);},
	    		Err(e) => error!("couldn't get client: {:?}", e),
	    	}
	    }
	}

	fn handle_client(&mut self, stream: TcpStream) {
		self.peer_address = stream.peer_addr().unwrap();
		self.local_address = stream.local_addr().unwrap();
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
					self.handle_nick(&mut stream, nick);
				},
				Ok(Command::User(user)) => {
					self.handle_user(&mut stream, user);
				},
				Ok(Command::Quit(quit_message)) => {
					self.handle_quit(&mut stream, quit_message);
					break;
				}
				Err(e) => {
					error!("Message Parsing Error: {}", e);
				},
			}
		}
	}

	fn handle_nick(&mut self, stream: &mut BufStream<TcpStream>,
		nick: String) {
		trace!("got NICK message\nnick: {}", nick);
		self.nicknames.insert(self.peer_address, nick);
		if self.users.contains_key(&self.peer_address) {
			self.send_rpl_welcome(stream);
		}
	}

	fn handle_user(&mut self, stream: &mut BufStream<TcpStream>,
		user: User) {
		trace!("got USER message\nuser: {}\nmode: {}\nrealname: {}",
			user.user, user.mode, user.realname);
		self.users.insert(self.peer_address, user);
		if self.nicknames.contains_key(&self.peer_address) {
			self.send_rpl_welcome(stream);
		}
	}

	fn handle_quit(&mut self, stream: &mut BufStream<TcpStream>,
		quit_message: String) {
		trace!("got QUIT message\nquit_message: {}", quit_message);
		self.send_rpl_quit(stream, quit_message);
	}

	fn send_rpl_welcome(&self, stream: &mut BufStream<TcpStream>) {
		if let Err(e) = write!(stream, ":{} 001 {} :Welcome to the Internet Relay Network {}!{}@{}\r\n",
				self.local_address,
				self.nicknames[&self.peer_address],
				self.nicknames[&self.peer_address],
				self.users[&self.peer_address].user,
				self.peer_address) {
			error!("Stream Write Error: {}", e);
		}
		if let Err(e) = stream.flush() {
			error!("Stream Flush Error: {}", e);
		}
	}

	fn send_rpl_quit(&self, stream: &mut BufStream<TcpStream>,
		quit_message: String) {
		if let Err(e) = write!(stream, "Closing Link: {} ({})",
				self.peer_address,
				quit_message) {
			error!("Stream Write Error: {}", e);
		}
		if let Err(e) = stream.flush() {
			error!("Stream Flush Error: {}", e);
		}
	}
}