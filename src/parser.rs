pub enum Message {
	Nick(String), // nickname
	User(User), // user, mode, realname
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

pub fn parse_message(message: String) -> Result<Message, &'static str> {
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
				return Ok(Message::Nick(msg_parts[param_index].to_string()));
			}
		},
		"USER" => {
			if num_param != 4 {
				return Err("USER needs 4 parameters");
			} else {
				return Ok(Message::User(
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