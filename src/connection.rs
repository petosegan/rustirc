use std::net::{TcpStream, SocketAddr};
use std::io::{Write, BufRead};
use bufstream::BufStream;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::sync::mpsc;
use std::fs::File;
use std::io::prelude::*;
use std::str;

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
	num_known_users: Arc<Mutex<usize>>,
}

impl Connection {
	pub fn new(stream: TcpStream,
		nicknames: Arc<Mutex<HashMap<String, SocketAddr>>>,
		users: Arc<Mutex<HashMap<SocketAddr, User>>>,
		rx: mpsc::Receiver<String>,
		phonebook: Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<String>>>>,
		num_known_users: Arc<Mutex<usize>>) -> Self {
		Connection {
			my_nickname: None,
			nicknames: nicknames,
			users: users,
			local_addr: stream.local_addr().unwrap(),
			peer_addr: stream.peer_addr().unwrap(),
			stream: BufStream::new(stream),
			rx: rx,
			phonebook: phonebook,
			num_known_users: num_known_users}
	}

	pub fn handle_client(&mut self) {
		loop {
			if let Ok(message) = self.rx.try_recv() {
				self.write_reply(format!("{}\r\n", message));
			}

			let mut buffer = String::new();

			if let Err(_) = self.stream.read_line(&mut buffer) {
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
					Ok(Command::Privmsg(target, text)) => { self.handle_privmsg(target, text); },
					Ok(Command::Notice(target, text)) => { self.handle_notice(target, text); },
					Ok(Command::Ping) => { self.handle_ping(); },
					Ok(Command::Pong) => {},
					Ok(Command::Motd) => { self.handle_motd(); },
					Ok(Command::Lusers) => { self.handle_lusers(); },
					Ok(Command::Whois(target)) => { self.handle_whois(target); },
					Ok(Command::Unknown(cmd)) => {self.send_err_unknowncommand(cmd); },
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
				self.send_welcome();
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
			self.send_welcome();
		}
	}

	fn send_welcome(&mut self) {
		{
			let mut n_users = self.num_known_users.lock().unwrap();
			(*n_users) += 1;
		}
		self.send_rpl_welcome();
		self.send_rpl_yourhost();
		self.send_rpl_created();
		self.send_rpl_myinfo();
		self.handle_lusers();
		self.handle_motd();
	}

	fn handle_quit(&mut self, quit_message: String) {
		trace!("got QUIT message\nquit_message: {}", quit_message);
		{ // remove self from shared data structures
			let mut nn = self.nicknames.lock().unwrap();
			let mut pb = self.phonebook.lock().unwrap();
			let mut uu = self.users.lock().unwrap();
			let target = self.get_nickname();
			let target_addr = (*nn)[&target];
			
			if let Some(_) = (*nn).remove(&target) {
				if let Some(_) = (*uu).remove(&target_addr) {
					let mut n_users = self.num_known_users.lock().unwrap();
					(*n_users) -= 1;
				}
			} else {
				(*uu).remove(&target_addr);
			}
			(*pb).remove(&target_addr);
		}

		self.send_rpl_quit(quit_message);
	}

	fn handle_privmsg(&mut self, target: String, text: String) {
		trace!("got PRIVMSG message\ntarget: {}\ntext: {}", target, text);
		let mut target_exists = false;
		{
			let nn = self.nicknames.lock().unwrap();
			if let Some(_) = (*nn).get(&target) {
				target_exists = true;
			}
		}
		if target_exists {
			let nn = self.nicknames.lock().unwrap();
			let pb = self.phonebook.lock().unwrap();
			let target_addr = (*nn)[&target];
			let target_tx = &(*pb)[&target_addr];

			let prefix = format!(":{}!{}@{} PRIVMSG {} :", self.get_nickname(), self.get_user(), self.local_addr, target);
			let full_message = format!("{}{}", prefix, text);

			(*target_tx).send(full_message).unwrap();
		} else {
			self.send_err_nosuchnick(target);
		}
	}

	fn handle_notice(&mut self, target: String, text: String) {
		trace!("got NOTICE message\ntarget: {}\ntext: {}", target, text);
		let mut target_exists = false;
		{
			let nn = self.nicknames.lock().unwrap();
			if let Some(_) = (*nn).get(&target) {
				target_exists = true;
			}
		}
		if target_exists {
			let nn = self.nicknames.lock().unwrap();
			let pb = self.phonebook.lock().unwrap();
			let target_addr = (*nn)[&target];
			let target_tx = &(*pb)[&target_addr];

			let prefix = format!(":{}!{}@{} NOTICE {} :", self.get_nickname(), self.get_user(), self.local_addr, target);
			let full_message = format!("{}{}", prefix, text);

			(*target_tx).send(full_message).unwrap();
		}
	}

	fn handle_ping(&mut self) {
		let reply = format!("PONG {}\r\n", self.local_addr);
		self.write_reply(reply);
	}

	fn handle_motd(&mut self) {
		let f_result = File::open("motd.txt");
		if let Err(_) = f_result {
			self.send_err_nomotd();
		} else {
			self.send_rpl_motd_start();
			let mut buffer = [0; 80];
			let mut f = f_result.unwrap();
			loop {
				let fread = f.read(&mut buffer);
				match fread {
					Ok(n) if n > 0 => {
						let reply = format!(":{} 372 {} :- {}\r\n", 
							self.local_addr, 
							self.get_nickname(),
							str::from_utf8(&buffer[..n]).unwrap());
						self.write_reply(reply);
					},
					_ => { break; }
				}
			}
			self.send_rpl_motd_end();
		}
	}

	fn handle_lusers(&mut self) {
		self.send_rpl_luserclient();
		self.send_rpl_luserop();
		self.send_rpl_luserunknown();
		self.send_rpl_luserchannels();
		self.send_rpl_luserme();
	}

	fn handle_whois(&mut self, target: String) {
		let mut target_exists = false;
		{
			let nn = self.nicknames.lock().unwrap();
			if let Some(_) = (*nn).get(&target) {
				target_exists = true;
			}
		}
		if target_exists {
			let target_user;
			let target_addr;
			{
				let nn = self.nicknames.lock().unwrap();
				let uu = self.users.lock().unwrap();
				target_addr = (*nn)[&target].clone();
				target_user = (*uu)[&target_addr].clone();
			}
			self.send_rpl_whoisuser(target.clone(), target_user, target_addr);
			self.send_rpl_whoisserver(target.clone(), target_addr);
			self.send_rpl_endofwhois(target.clone());
		} else {
			self.send_err_nosuchnick(target);
		}
	}

	fn send_rpl_motd_start(&mut self) {
		let reply = format!(":{} 375 {} :- {} Message of the day - \r\n", 
			self.local_addr,
			self.get_nickname(),
			self.local_addr);
		self.write_reply(reply);
	}

	fn send_rpl_motd_end(&mut self) {
		let reply = format!(":{} 376 {} :End of MOTD command\r\n",
			self.local_addr,
			self.get_nickname());
		self.write_reply(reply);
	}

	fn send_rpl_welcome(&mut self) {
		let this_nickname = self.get_nickname();
		let this_user = self.get_user();
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

	fn send_rpl_luserclient(&mut self) {
		let reply = format!(":{} 251 {} :There are {} users and 0 services on 1 servers\r\n",
			self.local_addr,
			self.get_nickname(),
			self.get_num_users());
		self.write_reply(reply);
	}

	fn send_rpl_luserop(&mut self) {
		let this_nickname = self.get_nickname();
		let reply = format!(":{} 252 {} 0 :operator(s) online\r\n",
			self.local_addr,
			this_nickname);
		self.write_reply(reply);
	}

	fn send_rpl_luserunknown(&mut self) {
		let this_nickname = self.get_nickname();
		let reply = format!(":{} 253 {} {} :unknown connection(s)\r\n",
			self.local_addr,
			this_nickname,
			self.get_num_unknown());
		self.write_reply(reply);
	}

	fn send_rpl_luserchannels(&mut self) {
		let this_nickname = self.get_nickname();
		let reply = format!(":{} 254 {} 0 :channels formed\r\n",
			self.local_addr,
			this_nickname);
		self.write_reply(reply);
	}

	fn send_rpl_luserme(&mut self) {
		let this_nickname = self.get_nickname();
		let reply = format!(":{} 255 {} :I have {} clients and 1 servers\r\n",
			self.local_addr,
			this_nickname,
			self.get_num_clients());
		self.write_reply(reply);
	}

	fn send_rpl_whoisuser(&mut self, nick: String, user: User, host: SocketAddr) {
		let this_nickname = self.get_nickname();
		let reply = format!(":{} 311 {} {} {} {} * :{}\r\n",
			self.local_addr,
			this_nickname,
			nick,
			user.user,
			host,
			user.realname);
		self.write_reply(reply);
	}

	fn send_rpl_whoisserver(&mut self, nick: String, host:SocketAddr) {
		let this_nickname = self.get_nickname();
		let reply = format!(":{} 312 {} {} {} :server info\r\n",
			self.local_addr,
			this_nickname,
			nick,
			host);
		self.write_reply(reply);
	}

	fn send_rpl_endofwhois(&mut self, nick: String) {
		let this_nickname = self.get_nickname();
		let reply = format!(":{} 318 {} {} :End of WHOIS list\r\n",
			self.local_addr,
			this_nickname,
			nick);
		self.write_reply(reply);
	}

	fn send_err_nicknameinuse(&mut self, nickname: String) {
		let reply = format!(":{} 433 * {} :Nickname is already in use\r\n",
				self.local_addr,
				nickname);
		self.write_reply(reply);
	}

	fn send_err_nosuchnick(&mut self, nickname: String) {
		let reply = format!(":{} 401 {} {} :No such nick/channel\r\n",
			self.local_addr,
			self.get_nickname(),
			nickname);
		self.write_reply(reply);
	}

	fn send_err_nomotd(&mut self) {
		let reply = format!(":{} 422 {} :MOTD File is missing\r\n",
			self.local_addr,
			self.get_nickname());
		self.write_reply(reply);
	}

	fn send_err_unknowncommand(&mut self, cmd: String) {
		let reply = format!(":{} 421 {} {} :Unknown command\r\n",
			self.local_addr,
			self.get_nickname(),
			cmd);
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

	fn get_user(&self) -> String {
		let uu = self.users.lock().unwrap();
		return (*uu)[&self.peer_addr].user.clone();
	}

	fn get_num_users(&self) -> usize {
		let n_users = self.num_known_users.lock().unwrap();
		return n_users.clone();
	}

	fn get_num_unknown(&self) -> usize {
		return self.get_num_clients() - self.get_num_users();
	}

	fn get_num_clients(&self) -> usize {
		let pb = self.phonebook.lock().unwrap();
		return (*pb).len();
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