use std::{collections::HashMap, error::Error};

use crate::wayland::{CtxType, WaylandError, WaylandObjectKind, display::Display, wire::{Id, WireArgument, WireEventRaw, WireRequest}};

pub struct Registry {
	id: Id,
	inner: HashMap<u32, RegistryEntry>,
	ctx: CtxType,
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct RegistryEntry {
	interface: String,
	version: u32,
}

impl Registry {
	pub fn new_empty(id: Id, ctx: CtxType) -> Self {
		Self {
			id,
			inner: HashMap::new(),
			ctx,
		}
	}

	pub fn new_filled(
		display: &mut Display,
		ctx: CtxType,
	) -> Result<Self, Box<dyn Error>> {
		let reg_id = display.wl_get_registry()?;
		let cbid = display.wl_sync()?;

		let mut events = vec![];
		let mut done = false;
		while !done {
			ctx.borrow_mut().wlmm.get_events()?;

			while let Some(msg) = ctx.borrow_mut().wlmm.q.pop_front() {
				if msg.recv_id == cbid {
					println!("registry callback done");
					done = true;
					break;
				} else if msg.recv_id == reg_id {
					events.push(msg);
				}
			}
		}

		let mut registry = Self::new_empty(reg_id, ctx);
		registry.fill(&events)?;
		Ok(registry)
	}

	pub(super) fn wl_bind(
		&mut self,
		id: Id,
		object: WaylandObjectKind,
		version: u32,
	) -> Result<(), Box<dyn Error>> {
		let global_id = self
			.inner
			.iter()
			.find(|(_, v)| v.interface == object.as_str())
			.map(|(k, _)| k)
			.copied()
			.ok_or(WaylandError::NotInRegistry)?;
		println!("bind global id for {}: {}", object.as_str(), global_id);

		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			// wl_registry id
			sender_id: self.id,
			// first request in the proto
			opcode: 0,
			args: vec![
				WireArgument::UnInt(global_id),
				// WireArgument::NewId(new_id),
				WireArgument::NewIdSpecific(object.as_str(), version, id),
			],
		})?;
		Ok(())
	}

	pub fn fill(&mut self, events: &[WireEventRaw]) -> Result<(), Box<dyn Error>> {
		for e in events {
			if e.recv_id != self.id {
				continue;
			};
			let name;
			let interface;
			let version;
			if let WireArgument::UnInt(name_) = e.args[0] {
				name = name_;
			} else {
				return Err(WaylandError::ParseError.boxed());
			};
			if let WireArgument::String(interface_) = &e.args[1] {
				interface = interface_.clone();
			} else {
				return Err(WaylandError::ParseError.boxed());
			};
			if let WireArgument::UnInt(version_) = e.args[2] {
				version = version_;
			} else {
				return Err(WaylandError::ParseError.boxed());
			};

			self.inner.insert(
				name,
				RegistryEntry {
					interface,
					version,
				},
			);
		}
		Ok(())
	}

	pub fn does_implement(&self, query: &str) -> Option<u32> {
		self.inner.iter().find(|(_, v)| v.interface == query).map(|(_, v)| v.version)
	}
}
