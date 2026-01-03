use std::{collections::HashMap, error::Error, fmt};

use crate::wayland::wire::{MessageManager, WireArgument, WireMessage};

pub mod wire;

pub struct Display {
	pub id: u32,
}

impl Display {
	pub fn new(wlim: &mut IdManager) -> Self {
		Self { id: wlim.new_id() }
	}

	pub fn wl_get_registry(
		&mut self,
		wlmm: &mut MessageManager,
		wlim: &mut IdManager,
	) -> Result<u32, Box<dyn Error>> {
		let id = wlim.new_id();
		wlmm.send_request(&mut WireMessage {
			sender_id: self.id,
			// second request in the proto
			opcode: 1,
			args: vec![
				// wl_registry id is now 2 since 1 is the display
				WireArgument::NewId(id),
			],
		})?;
		Ok(id)
	}

	pub fn wl_sync(&mut self, wlmm: &mut MessageManager, wlim: &mut IdManager) -> Result<(), Box<dyn Error>> {
		wlmm.send_request(&mut WireMessage {
			sender_id: self.id,
			opcode: 0,
			args: vec![WireArgument::NewId(wlim.new_id())],
		})
	}
}

pub struct Registry<'a> {
	pub id: u32,
	pub inner: HashMap<u32, RegistryEntry<'a>>,
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct RegistryEntry<'a> {
	pub interface: &'a str,
	version: u32,
}

impl<'a> Registry<'a> {
	pub fn new(id: u32) -> Self {
		Self {
			id,
			inner: HashMap::new(),
		}
	}

	pub fn wl_bind(
		&mut self,
		wlmm: &mut MessageManager,
		wlim: &mut IdManager,
		object: WaylandObject,
	) -> Result<u32, Box<dyn Error>> {
		let global_id = self
			.inner
			.iter()
			.find(|(_, v)| v.interface == object.as_str())
			.map(|(k, _)| k)
			.copied()
			.ok_or(WaylandError::NotInRegistry)?;
		let new_id = wlim.new_id();

		wlmm.send_request(&mut WireMessage {
			// wl_registry id
			sender_id: self.id,
			// first request in the proto
			opcode: 0,
			args: vec![WireArgument::UnInt(global_id), WireArgument::NewId(new_id)],
		})?;

		Ok(new_id)
	}

	pub fn fill(&mut self, events: &'a Vec<WireMessage>) -> Result<(), Box<dyn Error>> {
		for e in events {
			if e.sender_id != self.id {
				continue;
			};
			// println!("in fill ========\n{:#?}", e);
			let name;
			let interface;
			let version;
			if let WireArgument::UnInt(name_) = e.args[0] {
				name = name_;
			} else {
				return Err(Box::new(WaylandError::ParseError));
			};
			if let WireArgument::String(interface_) = &e.args[1] {
				interface = interface_;
			} else {
				return Err(Box::new(WaylandError::ParseError));
			};
			if let WireArgument::UnInt(version_) = e.args[2] {
				version = version_;
			} else {
				return Err(Box::new(WaylandError::ParseError));
			};

			self.inner
				.insert(name, RegistryEntry { interface, version });
		}
		Ok(())
	}
}

#[derive(PartialEq)]
pub enum WaylandObject {
	Display,
	Registry,
	Compositor,
}

impl WaylandObject {
	fn as_str(&self) -> &'static str {
		match self {
			WaylandObject::Display => "wl_display",
			WaylandObject::Registry => "wl_registry",
			WaylandObject::Compositor => "wl_compositor",
		}
	}
}

#[derive(Default)]
pub struct IdManager {
	top_id: u32,
	map: HashMap<WaylandObject, u32>,
}

impl IdManager {
	pub fn new_id(&mut self) -> u32 {
		self.top_id += 1;
		println!("new id called, new id is {}", self.top_id);
		self.top_id
	}

	fn get_object_id(&self, obj: WaylandObject) -> Option<u32> {
		self.map
			.iter()
			.find(|(k, _)| **k == obj)
			.map(|(_, v)| v)
			.copied()
	}
}

#[derive(Debug)]
pub enum WaylandError {
	ParseError,
	RecvLenBad,
	NotInRegistry,
}

impl fmt::Display for WaylandError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			WaylandError::ParseError => write!(f, "parse error"),
			WaylandError::RecvLenBad => write!(f, "received len is bad"),
			WaylandError::NotInRegistry => write!(f, "given name was not found in the registry hashmap"),
		}
	}
}

impl Error for WaylandError {
}
