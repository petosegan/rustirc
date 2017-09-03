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
				Ok(Command::Nick(nick)) => { self.handle_nick(nick); },
				Ok(Command::User(user)) => { self.handle_user(user); },
				Ok(Command::Quit(quit_message)) => {
					self.handle_quit(quit_message);
					break;
				}
				Err(e) => { error!("Message Parsing Error: {}", e); },
			}
		}
	}

	fn handle_nick(&mut self, nick: String) {
		trace!("got NICK message\nnick: {}", nick);

		let does_contain : bool;
		{
			let nn = self.nicknames.lock().unwrap();
			does_contain = (*nn).values()
		        .find(|&val| *val == nick)
		        .is_some();
		}

		if does_contain { 
	    	self.send_err_nicknameinuse(nick);
	    } else {

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
				self.send_rpl_yourhost();
				self.send_rpl_created();
				self.send_rpl_myinfo();
				self.send_rpl_postwelcome();
			}
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
			self.send_rpl_yourhost();
			self.send_rpl_created();
			self.send_rpl_myinfo();
			self.send_rpl_postwelcome();
		}
	}

	fn handle_quit(&mut self, quit_message: String) {
		trace!("got QUIT message\nquit_message: {}", quit_message);
		self.send_rpl_quit(quit_message);
	}

	fn send_rpl_welcome(&mut self) {
		let this_nickname = self.get_nickname();
		let this_user: String;
		{
			let uu = self.users.lock().unwrap();
			this_user = (*uu)[&self.peer_addr].user.clone();
		}
		let reply = format!(":{} 001 {} :Welcome to the Internet Relay Network {}!{}@{}\r\n",
				self.local_addr,
				this_nickname,
				this_nickname,
				this_user,
				self.peer_addr);
		self.write_reply(reply);
	}

	fn send_rpl_quit(&mut self, quit_message: String) {
		let reply = format!("ERROR :Closing Link: {} ({})\r\n",
				self.peer_addr,
				quit_message);
		self.write_reply(reply);
	}

	fn send_rpl_yourhost(&mut self) {
		let this_nickname = self.get_nickname();
		let reply = format!(":{} 002 {} :Your host is {}, running version 0.1\r\n",
				self.local_addr,
				this_nickname,
				self.local_addr);
		self.write_reply(reply);
	}

	fn send_rpl_created(&mut self) {
		let this_nickname = self.get_nickname();
		let reply = format!(":{} 003 {} :This server was created SOMEDATE\r\n",
				self.local_addr,
				this_nickname);
		self.write_reply(reply);
	}

	fn send_rpl_myinfo(&mut self) {
		let this_nickname = self.get_nickname();
		let reply = format!(":{} 004 {} {} 0.1 ao mtov\r\n",
				self.local_addr,
				this_nickname,
				self.local_addr);
		self.write_reply(reply);
	}

	fn send_rpl_postwelcome(&mut self) {
		let this_nickname = self.get_nickname();
		let reply = format!(":{} 251 {} :There are 1 users and 0 services on 1 servers\r\n:{} 252 {} 0 :operator(s) online\r\n:{} 253 {} 0 :unknown connection(s)\r\n:{} 254 {} 0 :channels formed\r\n:{} 255 {} :I have 1 clients and 1 servers\r\n:{} 422 {} :MOTD File is missing\r\n",
				self.local_addr,
				this_nickname,
				self.local_addr,
				this_nickname,
				self.local_addr,
				this_nickname,
				self.local_addr,
				this_nickname,
				self.local_addr,
				this_nickname,
				self.local_addr,
				this_nickname);
		self.write_reply(reply);
	}

	fn send_err_nicknameinuse(&mut self, nickname: String) {
		let reply = format!(":{} 433 * {} :Nickname is already in use\r\n",
				self.local_addr,
				nickname);
		self.write_reply(reply);
	}

	// fn send_err_alreadyregistered(&mut self) {
	// 	let reply = format!(":{} 462 :Unauthorized command (already registered)\r\n",
	// 			self.local_addr);
	// 	self.write_reply(reply);
	// }

	fn get_nickname(&self) -> String {
		let nn = self.nicknames.lock().unwrap();
		return (*nn)[&self.peer_addr].clone();
	}

	fn write_reply(&mut self, reply: String) {
		if let Err(e) = self.stream.write(reply.as_bytes()) {
			error!("Stream Write Error: {}", e);
		}
		if let Err(e) = self.stream.flush() {
			error!("Stream Flush Error: {}", e);
		}
	}
}