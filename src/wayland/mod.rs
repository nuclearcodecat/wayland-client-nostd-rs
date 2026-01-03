use std::{collections::HashMap, error::Error, ffi::CString, fmt};

// std depends on libc anyway so i consider using it fair
use libc::{O_CREAT, O_RDWR, shm_open, shm_unlink};

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

	pub fn wl_sync(
		&mut self,
		wlmm: &mut MessageManager,
		wlim: &mut IdManager,
	) -> Result<u32, Box<dyn Error>> {
		let cb_id = wlim.new_id();
		wlmm.send_request(&mut WireMessage {
			sender_id: self.id,
			opcode: 0,
			args: vec![WireArgument::NewId(cb_id)],
		})?;
		Ok(cb_id)
	}
}

pub struct Registry {
	pub id: u32,
	pub inner: HashMap<u32, RegistryEntry>,
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct RegistryEntry {
	pub interface: String,
	version: u32,
}

impl Registry {
	pub fn new_empty(id: u32) -> Self {
		Self {
			id,
			inner: HashMap::new(),
		}
	}

	pub fn new_bound_filled(
		display: &mut Display,
		wlmm: &mut MessageManager,
		wlim: &mut IdManager,
	) -> Result<Self, Box<dyn Error>> {
		let reg_id = display.wl_get_registry(wlmm, wlim)?;
		let mut registry = Self::new_empty(reg_id);

		let read = wlmm.get_events_blocking(registry.id, WaylandObjectKind::Registry)?;
		registry.fill(&read)?;
		Ok(registry)
	}

	pub fn wl_bind(
		&mut self,
		wlmm: &mut MessageManager,
		wlim: &mut IdManager,
		object: WaylandObjectKind,
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

	pub fn fill(&mut self, events: &[WireMessage]) -> Result<(), Box<dyn Error>> {
		for e in events {
			if e.sender_id != self.id {
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

			self.inner
				.insert(name, RegistryEntry { interface, version });
		}
		Ok(())
	}
}

pub struct Compositor {
	pub id: u32,
}

impl Compositor {
	pub fn new(id: u32) -> Self {
		Self { id }
	}

	pub fn new_bound(
		registry: &mut Registry,
		wlmm: &mut MessageManager,
		wlim: &mut IdManager,
	) -> Result<Self, Box<dyn Error>> {
		let id = registry.wl_bind(wlmm, wlim, WaylandObjectKind::Compositor)?;
		Ok(Self::new(id))
	}

	pub fn wl_create_surface(
		&self,
		wlmm: &mut MessageManager,
		wlim: &mut IdManager,
	) -> Result<u32, Box<dyn Error>> {
		let id = wlim.new_id();
		wlmm.send_request(&mut WireMessage {
			sender_id: self.id,
			opcode: 0,
			args: vec![WireArgument::UnInt(id)],
		})?;
		Ok(id)
	}
}

pub struct SharedMemory {
	id: u32,
}

impl SharedMemory {
	pub fn new(id: u32) -> Self {
		Self { id }
	}

	pub fn new_bound(
		registry: &mut Registry,
		wlmm: &mut MessageManager,
		wlim: &mut IdManager,
	) -> Result<Self, Box<dyn Error>> {
		let id = registry.wl_bind(wlmm, wlim, WaylandObjectKind::SharedMemory)?;
		Ok(Self::new(id))
	}

	pub fn wl_create_pool(
		&self,
		name: &CString,
		wlmm: &mut MessageManager,
		wlim: &mut IdManager,
	) -> Result<u32, Box<dyn Error>> {
		let fd = unsafe { shm_open(name.as_ptr(), O_RDWR | O_CREAT, 0) };
		println!("fd: {}", fd);

		let id = wlim.new_id();
		wlmm.send_request(&mut WireMessage {
			sender_id: self.id,
			opcode: 0,
			args: vec![
				WireArgument::NewId(id),
				// fd,
			],
		})?;
		Ok(id)
	}
}

pub struct SharedMemoryPool {
	id: u32,
	name: CString,
	size: usize,
}

impl SharedMemoryPool {
	pub fn new(id: u32, name: CString, size: usize) -> Self {
		Self { id, name, size }
	}

	pub fn new_bound(
		shm: &mut SharedMemory,
		size: usize,
		wlmm: &mut MessageManager,
		wlim: &mut IdManager,
	) -> Result<Self, Box<dyn Error>> {
		let name = CString::new("wl-shm-1")?;
		let id = shm.wl_create_pool(&name, wlmm, wlim)?;
		Ok(Self::new(id, name, size))
	}

	fn wl_release(
		&self,
		wlmm: &mut MessageManager,
	) -> Result<(), Box<dyn Error>> {
		wlmm.send_request(&mut WireMessage {
			sender_id: self.id,
			opcode: 1,
			args: vec![],
		})
	}

	fn unlink(&self) -> Result<(), std::io::Error> {
		let r = unsafe { shm_unlink(self.name.as_ptr()) };
		if r == 0 {
			Ok(())
		} else {
			Err(std::io::Error::last_os_error())
		}
	}

	pub fn destroy(&self, wlmm: &mut MessageManager, wlim: &mut IdManager) -> Result<(), Box<dyn Error>> {
		self.wl_release(wlmm)?;
		wlim.free_id(self.id);
		Ok(self.unlink()?)
	}
}

impl Drop for SharedMemoryPool {
	fn drop(&mut self) {
		panic!("called drop for SharedMemoryPool, use destroy");
		// println!("called drop for SharedMemoryPool");
		// if let Err(r) = self.unlink() {
		// 	eprintln!("failed to drop SharedMemoryPool\n{:#?}", r);
		// }
	}
}


#[derive(PartialEq)]
pub enum WaylandObjectKind {
	Display,
	Registry,
	Callback,
	Compositor,
	SharedMemory,
	SharedMemoryPool,
	Buffer,
}

impl WaylandObjectKind {
	fn as_str(&self) -> &'static str {
		match self {
			WaylandObjectKind::Display => "wl_display",
			WaylandObjectKind::Registry => "wl_registry",
			WaylandObjectKind::Callback => "wl_callback",
			WaylandObjectKind::Compositor => "wl_compositor",
			WaylandObjectKind::SharedMemory => "wl_shm",
			WaylandObjectKind::SharedMemoryPool => "wl_shm_pool",
			WaylandObjectKind::Buffer => "wl_buffer",
		}
	}
}

#[derive(Default)]
pub struct IdManager {
	top_id: u32,
	free: Vec<u32>,
}

impl IdManager {
	fn new_id(&mut self) -> u32 {
		self.top_id += 1;
		self.top_id
	}

	fn free_id(&mut self, id: u32) {
		self.free.push(id);
	}
}

#[derive(Debug)]
pub enum WaylandError {
	ParseError,
	RecvLenBad,
	NotInRegistry,
	ObjectNonExistent,
}

impl WaylandError {
	fn boxed(self) -> Box<Self> {
		Box::new(self)
	}
}

impl fmt::Display for WaylandError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			WaylandError::ParseError => write!(f, "parse error"),
			WaylandError::RecvLenBad => write!(f, "received len is bad"),
			WaylandError::NotInRegistry => {
				write!(f, "given name was not found in the registry hashmap")
			}
			WaylandError::ObjectNonExistent => write!(f, "requested object doesn't exist"),
		}
	}
}

impl Error for WaylandError {}
