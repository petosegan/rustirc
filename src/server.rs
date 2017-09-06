use std::net::{TcpListener, SocketAddr};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::mpsc;

use parser::{User};
use connection::{Connection};

pub struct IrcServer {
	nicknames: Arc<Mutex<HashMap<String, SocketAddr>>>, 
	users: Arc<Mutex<HashMap<SocketAddr, User>>>,
	phonebook: Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<String>>>>,
	portnum: u16,
}

impl IrcServer {
	pub fn new(portnum: u16) -> Self {
		IrcServer { 
			nicknames: Arc::new(Mutex::new(HashMap::new())),
			users: Arc::new(Mutex::new(HashMap::new())),
			phonebook: Arc::new(Mutex::new(HashMap::new())),
			portnum: portnum}
	}

	pub fn run(&mut self) {
		let listener = TcpListener::bind(("127.0.0.1", self.portnum)).unwrap();
	    for socket in listener.incoming() {
	    	match socket {
	    		Ok(stream) => {
	    			stream.set_nonblocking(true).expect("set_nonblocking call failed");
	    			let this_nicknames = self.nicknames.clone();
	    			let this_users = self.users.clone();
	    			let this_phonebook = self.phonebook.clone();
	    			let (tx, rx) = mpsc::channel();
	    			{
	    				let mut pb = self.phonebook.lock().unwrap();
	    				(*pb).insert(stream.peer_addr().unwrap(), tx);
	    			}

	    			thread::spawn(|| {
		    			let mut this_connection = Connection::new(stream, this_nicknames, this_users, rx, this_phonebook);
		    			this_connection.handle_client();
		    		});
	    		},
	    		Err(e) => error!("couldn't get client: {:?}", e),
	    	}
	    }
	}
}