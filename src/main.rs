#![allow(dead_code)]

use std::{env, io::Read};

use crate::wayland::Display;

mod wayland {
	use std::{env, io::Write, os::unix::net::UnixStream, path::PathBuf};

	pub struct Display {
		pub id: u32,
		pub sock: UnixStream,
	}

	#[derive(PartialEq)]
	pub enum WireArgument {
		Int(i32),
		UnInt(u32),
		// add actual type and helper funs
		FixedPrecision(u32),
		String(String),
		Obj(u32),
		NewId(u32),
		NewIdSpecific(String, u32, u32),
		Arr(Vec<u8>),
		FileDescriptor(u32),
	}

	impl WireArgument {
		// size in bytes
		pub fn size(&self) -> usize {
			match self {
				WireArgument::Int(_) => 4,
				WireArgument::UnInt(_) => 4,
				WireArgument::FixedPrecision(_) => 4,
				WireArgument::String(x) => x.len(),
				WireArgument::Obj(_) => 4,
				WireArgument::NewId(_) => 4,
				WireArgument::NewIdSpecific(x, _, _) => x.len() + 8,
				WireArgument::Arr(x) => x.len(),
				WireArgument::FileDescriptor(_) => 4,
			}
		}

		pub fn as_vec_u8(&self) -> Vec<u8> {
			match self {
				WireArgument::Int(x) => Vec::from(x.to_ne_bytes()),
				WireArgument::UnInt(x) => Vec::from(x.to_ne_bytes()),
				WireArgument::FixedPrecision(x) => Vec::from(x.to_ne_bytes()),
				WireArgument::String(x) => Vec::from(x.as_str()),
				WireArgument::Obj(x) => Vec::from(x.to_ne_bytes()),
				WireArgument::NewId(x) => Vec::from(x.to_ne_bytes()),
				WireArgument::NewIdSpecific(x, y, z) => {
					let mut complete: Vec<u8> = vec![];
					complete.append(&mut Vec::from(x.as_str()));
					complete.append(&mut Vec::from(y.to_ne_bytes()));
					complete.append(&mut Vec::from(z.to_ne_bytes()));
					complete
				},
				WireArgument::Arr(items) => items.clone(),
				WireArgument::FileDescriptor(x) => Vec::from(x.to_ne_bytes()),
			}
		}
	}

	pub struct WireMessage {
		pub sender_id: u32,
		pub opcode: usize,
		pub args: Vec<WireArgument>,
	}

	impl Display {
		pub fn new(sockname: &str) -> Result<Display, ()> {
			// let base = env::var("XDG_RUNTIME_DIR").unwrap_or("/run/user/1000".to_string()); 
			let base = env::var("XDG_RUNTIME_DIR").map_err(|_| {})?; 
			let mut base = PathBuf::from(base);
			base.push(sockname);
			let sock = UnixStream::connect(base).map_err(|_| {})?;
			Ok(Display {
				// wl_display has an id of 1
				id: 1,
				sock,
			})
		}

		pub fn discon(&self) -> Result<(), ()> {
			self.sock.shutdown(std::net::Shutdown::Both).map_err(|_| {})
		}

		fn send_request(&mut self, msg: &mut WireMessage) -> Result<(), ()> {
			let mut buf: Vec<u8> = vec![];
			buf.append(&mut Vec::from(msg.sender_id.to_ne_bytes()));
			let argsize = {
				// header is 8
				let mut complete = 8;
				for n in msg.args.iter() {
					let size = n.size();
					complete += size;
				}
				complete
			};
			let word2 = (argsize << 16) as u32 | (msg.opcode as u32 & 0x0000ffffu32);
			buf.append(&mut Vec::from(word2.to_ne_bytes()));
			for obj in msg.args.iter_mut() {
				match obj {
					WireArgument::Arr(x) => buf.append(x),
					_ => buf.append(&mut obj.as_vec_u8())
				}
			}
			self.sock.write_all(&buf).map_err(|_| {})?;
			Ok(())
		}

		pub fn wl_get_registry(&mut self) -> Result<(), ()> {
			self.send_request(&mut WireMessage {
				sender_id: self.id,
				// second request in the proto
				opcode: 1,
				args: vec![
					WireArgument::NewId(0),
					// WireArgument::NewIdSpecific("wl_registry".to_string(), 1, 0),
				],
			})
		}
	}
}

fn main() -> Result<(), ()> {
	// let display_name = env::var("WAYLAND_DISPLAY").unwrap_or("wayland-0".to_string());
	let display_name = env::var("WAYLAND_DISPLAY").map_err(|_| {})?;
	let mut display = Display::new(&display_name)?;
	display.wl_get_registry()?;
	let mut buf = String::new();
	display.sock.read_to_string(&mut buf).unwrap();
	println!("recv: {}", buf);

	display.discon()?;
	println!("good");
	Ok(())
}
