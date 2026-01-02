use std::collections::HashMap;

use crate::wayland::wire::{MessageManager, WireArgument, WireMessage};

pub mod wire;

pub struct Display {
	pub id: u32,
}

impl Display {
	pub fn new(wlim: &mut IdManager) -> Self {
		Self {
			id: wlim.new_id(),
		}
	}

	pub fn wl_get_registry(&mut self, wlmm: &mut MessageManager, wlim: &mut IdManager) -> Result<u32, ()> {
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

	pub fn wl_sync(&mut self, wlmm: &mut MessageManager, wlim: &mut IdManager) -> Result<(), ()> {
		wlmm.send_request(&mut WireMessage {
			sender_id: self.id,
			opcode: 0,
			args: vec![
				WireArgument::NewId(wlim.new_id()),
			],
		})
	}
}

pub struct Registry<'a> {
	pub id: u32,
	pub inner: HashMap<u32, RegistryEntry<'a>>,
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct RegistryEntry<'a> {
	interface: &'a str,
	version: u32,
}

impl<'a> Registry<'a> {
	pub fn new(id: u32) -> Self {
		Self {
			id,
			inner: HashMap::new(),
		}
	}

	pub fn wl_bind(&mut self, wlmm: &mut MessageManager, wlim: &mut IdManager) -> Result<(), ()> {
		wlmm.send_request(&mut WireMessage {
			// wl_registry id
			sender_id: self.id,
			// first request in the proto
			opcode: 0,
			args: vec![
				WireArgument::UnInt(self.id),
				WireArgument::NewId(wlim.new_id()),
			],
		})
	}

	pub fn fill(&mut self, events: &'a Vec<WireMessage>) -> Result<(), ()> {
		for e in events {
			// get from idman
			if e.sender_id != 2 {
				continue;
			};
			println!("in fill ========\n{:#?}", e);
			let name;
			let interface;
			let version;
			if let WireArgument::UnInt(name_) = e.args[0] {
				name = name_;
			} else {
				return Err(());
			};
			if let WireArgument::String(interface_) = &e.args[1] {
				interface = interface_;
			} else {
				return Err(());
			};
			if let WireArgument::UnInt(version_) = e.args[2] {
				version = version_;
			} else {
				return Err(());
			};

			self.inner.insert(name, RegistryEntry { interface, version });
		}
		Ok(())
	}
}

#[derive(Default)]
pub struct IdManager {
	top_id: u32,
}

impl IdManager {
	pub fn new_id(&mut self) -> u32 {
		self.top_id += 1;
		println!("new id called, new id is {}", self.top_id);
		self.top_id
	}
}

