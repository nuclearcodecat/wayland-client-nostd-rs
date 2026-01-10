use std::{
	cell::RefCell, collections::HashMap, error::Error, fmt, rc::Rc
};
use crate::wayland::wire::{Id, MessageManager};
pub mod wire;
pub mod display;
pub mod registry;
pub mod compositor;
pub mod surface;
pub mod shm;
pub mod buffer;
pub mod xdgshell;

pub type OpCode = u32;

trait WaylandObject {
	fn handle(&mut self, opcode: OpCode, payload: &[u8]) -> Result<(), Box<dyn Error>>;
}

pub type CtxType = Rc<RefCell<Context>>;

pub struct Context {
	wlmm: MessageManager,
	wlim: IdentManager,
}

impl Context {
	pub fn new(wlmm: MessageManager, wlim: IdentManager) -> Self {
		Self {
			wlmm,
			wlim,
		}
	}

	fn serialize_events(&mut self) -> Result<(), Box<dyn Error>> {
		while let Some(ev) =  self.wlmm.q.pop_front() {
		}
		todo!()
	}
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum WaylandObjectKind {
	Display,
	Registry,
	Callback,
	Compositor,
	Surface,
	SharedMemory,
	SharedMemoryPool,
	Buffer,
	XdgWmBase,
	XdgSurface,
	XdgTopLevel,
}

impl WaylandObjectKind {
	fn as_str(&self) -> &'static str {
		match self {
			WaylandObjectKind::Display => "wl_display",
			WaylandObjectKind::Registry => "wl_registry",
			WaylandObjectKind::Callback => "wl_callback",
			WaylandObjectKind::Compositor => "wl_compositor",
			WaylandObjectKind::Surface => "wl_surface",
			WaylandObjectKind::SharedMemory => "wl_shm",
			WaylandObjectKind::SharedMemoryPool => "wl_shm_pool",
			WaylandObjectKind::Buffer => "wl_buffer",
			WaylandObjectKind::XdgWmBase => "xdg_wm_base",
			WaylandObjectKind::XdgSurface => "xdg_surface",
			WaylandObjectKind::XdgTopLevel => "xdg_toplevel",
		}
	}
}

#[derive(Default)]
pub struct IdentManager {
	top_id: Id,
	free: Vec<Id>,
	idmap: HashMap<Id, (WaylandObjectKind, Rc<RefCell<dyn WaylandObject>>)>,
}

impl IdentManager {
	fn new_id(&mut self) -> Id {
		self.top_id += 1;
		println!("! idman ! new id picked: {}", self.top_id);
		self.top_id
	}

	fn new_id_registered(&mut self, kind: WaylandObjectKind) -> Id {
		let id = self.new_id();
		self.idmap.insert(id, kind);
		id
	}

	fn free_id(&mut self, id: Id) -> Result<(), Box<dyn Error>> {
		let registered = self.idmap.iter().find(|(k, _)| **k == id).map(|(k, _)| k).copied();
		if let Some(r) = registered {
			self.idmap.remove(&r).ok_or(WaylandError::IdMapRemovalFail.boxed())?;
		}
		self.free.push(id);
		Ok(())
	}

	// ugh
	pub fn find_obj_by_id(&self, id: Id) -> Option<&WaylandObjectKind> {
		self.idmap.iter().find(|(k, _)| **k == id).map(|(_, v)| v)
	}
}

#[derive(Debug)]
pub enum WaylandError {
	ParseError,
	RecvLenBad,
	NotInRegistry,
	IdMapRemovalFail,
	ObjectNonExistent,
	InvalidPixelFormat,
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
			WaylandError::IdMapRemovalFail => write!(f, "failed to remove from id man map"),
			WaylandError::ObjectNonExistent => write!(f, "object non existent"),
			WaylandError::InvalidPixelFormat => {
				write!(f, "an invalid pixel format has been recved")
			}
		}
	}
}

impl Error for WaylandError {}
