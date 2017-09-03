use std::net::{TcpStream, SocketAddr};
use std::io::{Write, BufRead};
use bufstream::BufStream;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use parser::{Command, User, parse_message};

pub struct Connection {
	nicknames: Arc<Mutex<HashMap<SocketAddr, String>>>,
	users: Arc<Mutex<HashMap<SocketAddr, User>>>,
	local_addr: SocketAddr,
	peer_addr: SocketAddr,
	stream: BufStream<TcpStream>,
}

impl Connection {
	pub fn new(stream: TcpStream,
		nicknames: Arc<Mutex<HashMap<SocketAddr, String>>>,
		users: Arc<Mutex<HashMap<SocketAddr, User>>>) -> Self {
		Connection {
			nicknames: nicknames,
			users: users,
			local_addr: stream.local_addr().unwrap(),
			peer_addr: stream.peer_addr().unwrap(),
			stream: BufStream::new(stream)}
	}

	pub fn handle_client(&mut self) {
		loop {
			let mut buffer = String::new();

			if let Err(e) = self.stream.read_line(&mut buffer) {
				error!("Stream Read Error: {}", e);
				continue;
			}

			if buffer.is_empty() { break; }

			match parse_message(buffer) {
				Ok(Command::Nick(nick)) => {
					self.handle_nick(nick);
				},
				Ok(Command::User(user)) => {
					self.handle_user(user);
				},
				Ok(Command::Quit(quit_message)) => {
					self.handle_quit(quit_message);
					break;
				}
				Err(e) => {
					error!("Message Parsing Error: {}", e);
				},
			}
		}
	}

	fn handle_nick(&mut self, nick: String) {
		trace!("got NICK message\nnick: {}", nick);
		{
			let mut nn = self.nicknames.lock().unwrap();
			(*nn).insert(self.peer_addr, nick);
		}

		let has_user : bool;
		{
			let uu = self.users.lock().unwrap();
			has_user = (*uu).contains_key(&self.peer_addr);
		}

		if has_user {
			self.send_rpl_welcome();
		}
	}

	fn handle_user(&mut self, user: User) {
		trace!("got USER message\nuser: {}\nmode: {}\nrealname: {}",
			user.user, user.mode, user.realname);

		{
			let mut uu = self.users.lock().unwrap();
			(*uu).insert(self.peer_addr, user);
		}

		let has_nick : bool;
		{
			let nn = self.nicknames.lock().unwrap();
			has_nick = (*nn).contains_key(&self.peer_addr);
		}

		if has_nick {
			self.send_rpl_welcome();
		}
	}

	fn handle_quit(&mut self, quit_message: String) {
		trace!("got QUIT message\nquit_message: {}", quit_message);
		self.send_rpl_quit(quit_message);
	}

	fn send_rpl_welcome(&mut self) {
		let this_nickname: String;
		let this_user: String;

		{
			let nn = self.nicknames.lock().unwrap();
			this_nickname = (*nn)[&self.peer_addr].clone();
			let uu = self.users.lock().unwrap();
			this_user = (*uu)[&self.peer_addr].user.clone();
		}

		if let Err(e) = write!(self.stream, ":{} 001 {} :Welcome to the Internet Relay Network {}!{}@{}\r\n",
				self.local_addr,
				this_nickname,
				this_nickname,
				this_user,
				self.peer_addr) {
			error!("Stream Write Error: {}", e);
		}
		if let Err(e) = self.stream.flush() {
			error!("Stream Flush Error: {}", e);
		}
	}

	fn send_rpl_quit(&mut self, quit_message: String) {
		if let Err(e) = write!(self.stream, "Closing Link: {} ({})",
				self.peer_addr,
				quit_message) {
			error!("Stream Write Error: {}", e);
		}
		if let Err(e) = self.stream.flush() {
			error!("Stream Flush Error: {}", e);
		}
	}
}