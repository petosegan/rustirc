use std::net::{TcpStream, SocketAddr};
use std::io::{Write, BufRead};
use bufstream::BufStream;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::sync::mpsc;

use parser::{Command, User, parse_message};

pub struct Connection {
	my_nickname: Option<String>,
	nicknames: Arc<Mutex<HashMap<String, SocketAddr>>>,
	users: Arc<Mutex<HashMap<SocketAddr, User>>>,
	local_addr: SocketAddr,
	peer_addr: SocketAddr,
	stream: BufStream<TcpStream>,
	rx: mpsc::Receiver<String>,
	phonebook: Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<String>>>>,
}

impl Connection {
	pub fn new(stream: TcpStream,
		nicknames: Arc<Mutex<HashMap<String, SocketAddr>>>,
		users: Arc<Mutex<HashMap<SocketAddr, User>>>,
		rx: mpsc::Receiver<String>,
		phonebook: Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<String>>>>) -> Self {
		Connection {
			my_nickname: None,
			nicknames: nicknames,
			users: users,
			local_addr: stream.local_addr().unwrap(),
			peer_addr: stream.peer_addr().unwrap(),
			stream: BufStream::new(stream),
			rx: rx,
			phonebook: phonebook}
	}

	pub fn handle_client(&mut self) {
		loop {
			if let Ok(message) = self.rx.try_recv() {
				self.write_reply(format!("{}\r\n", message));
			}

			let mut buffer = String::new();

			if let Err(e) = self.stream.read_line(&mut buffer) {
				// error!("Stream Read Error: {}", e);
				continue;
			} else {
				if buffer.is_empty() { break; }

				match parse_message(buffer) {
					Ok(Command::Nick(nick)) => { self.handle_nick(nick); },
					Ok(Command::User(user)) => { self.handle_user(user); },
					Ok(Command::Quit(quit_message)) => {
						self.handle_quit(quit_message);
						break;
					},
					Ok(Command::Privmsg(target, text)) => { self.handle_privmsg(target, text); }
					Err(e) => { error!("Message Parsing Error: {}", e); },
				}
			}
		}
	}

	fn handle_nick(&mut self, nick: String) {
		trace!("got NICK message\nnick: {}", nick);

		self.my_nickname = Some(nick.clone());

		let does_contain : bool;
		{
			let nn = self.nicknames.lock().unwrap();
			does_contain = (*nn).contains_key(&nick);
		}

		if does_contain { 
	    	self.send_err_nicknameinuse(nick);
	    } else {

			{
				let mut nn = self.nicknames.lock().unwrap();
				(*nn).insert(nick, self.peer_addr);
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
			has_nick = (*nn).values()
		        .find(|&val| *val == self.peer_addr)
		        .is_some();
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

	fn handle_privmsg(&mut self, target: String, text: String) {
		trace!("got PRIVMSG message\ntarget: {}\ntext: {}", target, text);
		let nn = self.nicknames.lock().unwrap();
		let pb = self.phonebook.lock().unwrap();
		let target_addr = (*nn)[&target];
		let target_tx = &(*pb)[&target_addr];
		(*target_tx).send(text).unwrap();
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
		let result = self.my_nickname.clone().unwrap();
		return result;
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