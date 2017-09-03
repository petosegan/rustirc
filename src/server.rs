use std::net::{TcpListener, SocketAddr};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use parser::{User};
use connection::{Connection};

pub struct IrcServer {
	nicknames: Arc<Mutex<HashMap<SocketAddr, String>>>, 
	users: Arc<Mutex<HashMap<SocketAddr, User>>>,
	portnum: u16,
}

impl IrcServer {
	pub fn new(portnum: u16) -> Self {
		IrcServer { 
			nicknames: Arc::new(Mutex::new(HashMap::new())),
			users: Arc::new(Mutex::new(HashMap::new())),
			portnum: portnum}
	}

	pub fn run(&mut self) {
		let listener = TcpListener::bind(("127.0.0.1", self.portnum)).unwrap();
	    for socket in listener.incoming() {
	    	match socket {
	    		Ok(stream) => {
	    			let this_nicknames = self.nicknames.clone();
	    			let this_users = self.users.clone();
	    			thread::spawn(|| {
		    			let mut this_connection = Connection::new(stream, this_nicknames, this_users);
		    			this_connection.handle_client();
		    		});
	    		},
	    		Err(e) => error!("couldn't get client: {:?}", e),
	    	}
	    }
	}
}