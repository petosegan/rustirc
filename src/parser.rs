pub enum Command {
	Nick(String), // nickname
	User(User), // user, mode, realname
	Quit(String) // Quit Message
}

struct Message {
	prefix: Option<String>,
	command: String,
	params: Vec<String>,
}

impl Message {
	pub fn new() -> Self {
		Message {prefix: None, command: String::new(), params: vec![]}
	}
}

pub struct User {
	pub user: String,
	pub mode: String,
	pub realname: String,
}

impl User {
	pub fn new(user: String, mode: String, realname: String) -> Self {
		User {user: user, mode: mode, realname: realname}
	}
}

fn parse_stream(stream: String) -> Result<Message, &'static str> {
	let stream = stream.trim_right();
	let mut ix = 0;
	let mut this_message = Message::new();
	if stream.as_bytes()[0] == b':' {
		if let Some(prefix_end) = stream.as_bytes().iter().position(|&c| c == b' ') {
			trace!("scanned prefix");
			this_message.prefix = Some(stream[1..prefix_end].to_string());
			ix += prefix_end+1;
			trace!("command at ix: {}", ix);
		} else {
			return Err("invalid prefix");
		}
	}
	if let Some(command_end) = stream[ix..].as_bytes().iter().position(|&c| c == b' ') {
		this_message.command = stream[ix..ix+command_end].to_string();
		trace!("scanned command: {}", this_message.command);
		ix += command_end + 1;
		trace!("first param at ix: {}", ix);
	} else {
		this_message.command = stream[ix..].to_string();
		trace!("scanned command: {}", this_message.command);
		ix = stream.len();
	}
	while ix < stream.len() {
		// long parameter
		if stream[ix..].as_bytes()[0] == b':' {
			trace!("scanned long param: {}", stream[ix+1..].to_string());
			this_message.params.push(stream[ix+1..].to_string());
			break;
		}
		if let Some(param_end) = stream[ix..].as_bytes().iter().position(|&c| c == b' ') {
			trace!("scanned param: {}", stream[ix..ix+param_end].to_string());
			this_message.params.push(stream[ix..ix+param_end].to_string());
			ix += param_end+1
		} else {
			trace!("scanned final param: {}", stream[ix..].to_string());
			this_message.params.push(stream[ix..].to_string());
			break;
		}
	}
	Ok(this_message)
}

pub fn parse_message(message: String) -> Result<Command, &'static str> {
	debug!("\n\nmessage: {}", message);
	
	let this_message = parse_stream(message)?;
	let num_param = this_message.params.len();
	
	debug!("command: {}", this_message.command);
	// debug!("msg has {} params", num_param);
	match this_message.command.as_str() {
		"NICK" => {
			if num_param != 1 {
				return Err("NICK needs 1 parameter");
			} else {
				let this_nick = this_message.params[0].clone();
				return Ok(Command::Nick(this_nick));
			}
		},
		"USER" => {
			if num_param != 4 {
				return Err("USER needs 4 parameters");
			} else {
				return Ok(Command::User(
					User::new(
						this_message.params[0].to_string(),
				 		this_message.params[1].to_string(),
				 		this_message.params[3].to_string())
					));
			}
		},
		"QUIT" => {
			if num_param == 0 {
				return Ok(Command::Quit("Client Quit".to_string()));
			} else if num_param == 1 {
				return Ok(Command::Quit(this_message.params[0].to_string()));
			} else {
				return Err("Quit needs at most one parameter");
			}
		}
		_ => {return Err("unknown command");}
	}
}