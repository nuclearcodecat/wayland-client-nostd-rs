use std::{
	cell::RefCell, collections::{HashMap, HashSet}, error::Error, ffi::CString, fmt, os::fd::RawFd, rc::Rc
};

// std depends on libc anyway so i consider using it fair
// i may replace this with asm in the future but that means amd64 only
use libc::{O_CREAT, O_RDWR, ftruncate, shm_open, shm_unlink};

use crate::wayland::wire::{MessageManager, WireArgument, WireEventRaw, WireRequest, Id};

pub mod wire;

pub type CtxType = Rc<RefCell<Context>>;

#[derive(Debug)]
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
}

pub struct Display {
	pub id: Id,
	ctx: CtxType,
}

impl Display {
	pub fn new(ctx: CtxType) -> Self {
		let id = ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::Display);
		Self {
			id,
			ctx,
		}
	}

	fn wl_get_registry(
		&mut self,
	) -> Result<u32, Box<dyn Error>> {
		let id = self.ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::Registry);
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 1,
			args: vec![
				WireArgument::NewId(id),
			],
		})?;
		Ok(id)
	}

	pub fn wl_sync(
		&mut self,
	) -> Result<u32, Box<dyn Error>> {
		let cb_id = self.ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::Callback);
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 0,
			args: vec![WireArgument::NewId(cb_id)],
		})?;
		Ok(cb_id)
	}
}

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

	fn wl_bind(
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

pub struct Compositor {
	pub id: Id,
	ctx: CtxType,
}

impl Compositor {
	pub fn new(id: Id, ctx: CtxType) -> Self {
		Self {
			id,
			ctx,
		}
	}

	pub fn new_bound(
		registry: &mut Registry,
		ctx: CtxType,
	) -> Result<Self, Box<dyn Error>> {
		let id = ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::Compositor);
		registry.wl_bind(id, WaylandObjectKind::Compositor, 5)?;
		Ok(Self::new(id, ctx))
	}

	fn wl_create_surface(
		&self,
		id: Id,
	) -> Result<(), Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 0,
			args: vec![WireArgument::UnInt(id)],
		})
	}

	pub fn make_surface(
		&self,
	) -> Result<Surface, Box<dyn Error>> {
		let id = self.ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::Surface);
		self.wl_create_surface(id)?;
		let ctx = self.ctx.clone();
		Ok(Surface {
			id,
			ctx,
			attached_buf: None,
		})
	}
}

pub struct Surface {
	pub id: Id,
	ctx: CtxType,
	attached_buf: Option<u32>,
}

impl Surface {
	fn wl_destroy(&self) -> Result<(), Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 0,
			args: vec![],
		})
	}

	pub fn destroy(
		&self,
	) -> Result<(), Box<dyn Error>> {
		self.wl_destroy()?;
		self.ctx.borrow_mut().wlim.free_id(self.id)?;
		Ok(())
	}

	fn wl_attach(&self, buf_id: Id) -> Result<(), Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 1,
			args: vec![
				WireArgument::Obj(buf_id),
				WireArgument::UnInt(0),
				WireArgument::UnInt(0),
			],
		})
	}

	pub fn attach_buffer(
		&mut self,
		to_att: u32,
	) -> Result<(), Box<dyn Error>> {
		self.attached_buf = Some(to_att);
		self.wl_attach(to_att)
	}

	fn wl_commit(
		&self,
	) -> Result<(), Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 6,
			args: vec![],
		})
	}

	pub fn commit(
		&self,
	) -> Result<(), Box<dyn Error>> {
		self.wl_commit()
	}
}

#[derive(Debug)]
pub struct SharedMemory {
	id: Id,
	ctx: CtxType,
	valid_pix_formats: HashSet<PixelFormat>,
}

impl SharedMemory {
	pub fn new(id: Id, ctx: CtxType) -> Self {
		Self {
			id,
			ctx,
			valid_pix_formats: HashSet::new(),
		}
	}

	fn push_pix_format(&mut self, pf: PixelFormat) {
		self.valid_pix_formats.insert(pf);
	}

	pub fn new_bound_initialized(
		display: &mut Display,
		registry: &mut Registry,
		ctx: CtxType,
	) -> Result<Self, Box<dyn Error>> {
		let id = ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::SharedMemory);
		registry.wl_bind(id, WaylandObjectKind::SharedMemory, 1)?;
		let mut shm = Self::new(id, ctx);

		let cbid = display.wl_sync()?;
		ctx.borrow_mut().wlmm.get_events()?;
		let mut done = false;
		while !done {
			while let Some(msg) = ctx.borrow_mut().wlmm.q.pop_front() {
				if msg.recv_id == id {
					for arg in msg.args {
						if let WireArgument::UnInt(x) = arg {
							let conv = PixelFormat::from_u32(x);
							if let Ok(pf) = conv {
								shm.push_pix_format(pf);
							} else {
								eprintln!("found unrecognized pixel format {:08x}", x);
							}
						}
					}
				} else if msg.recv_id == cbid {
					done = true;
				}
			}
		}

		// println!("shm\n{:#?}", shm);
		Ok(shm)
	}

	pub fn make_pool(
		&self,
		size: i32,
	) -> Result<SharedMemoryPool, Box<dyn Error>> { let name = CString::new("wl-shm-1")?;
		let (id, fd) = self.wl_create_pool(&name, size)?;
		Ok(SharedMemoryPool::new(id, self.ctx.clone(), name, size, fd))
	}

	fn wl_create_pool(
		&self,
		name: &CString,
		size: i32,
	) -> Result<(Id, RawFd), Box<dyn Error>> {
		let fd = unsafe { shm_open(name.as_ptr(), O_RDWR | O_CREAT, 0) };
		println!("fd: {}", fd);
		unsafe { ftruncate(fd, size.into()) };

		let id = self.ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::SharedMemoryPool);
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 0,
			args: vec![
				// WireArgument::NewIdSpecific(WaylandObjectKind::SharedMemoryPool.as_str(), 1, id),
				WireArgument::NewId(id),
				WireArgument::FileDescriptor(fd),
				WireArgument::Int(size),
			],
		})?;
		Ok((id, fd))
	}
}

pub struct SharedMemoryPool {
	id: Id,
	ctx: CtxType,
	name: CString,
	size: i32,
	fd: RawFd,
}

impl SharedMemoryPool {
	pub fn new(id: Id, ctx: CtxType, name: CString, size: i32, fd: RawFd) -> Self {
		Self {
			id,
			ctx,
			name,
			size,
			fd,
		}
	}

	fn wl_create_buffer(
		&self,
		(offset, width, height, stride): (i32, i32, i32, i32),
		format: PixelFormat,
	) -> Result<u32, Box<dyn Error>> {
		let id = self.ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::Buffer);
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 0,
			args: vec![
				WireArgument::NewId(id),
				WireArgument::Int(offset),
				WireArgument::Int(width),
				WireArgument::Int(height),
				WireArgument::Int(stride),
				WireArgument::UnInt(format as u32),
			],
		})?;
		Ok(id)
	}

	pub fn make_buffer(
		&self,
		(offset, width, height, stride): (i32, i32, i32, i32),
		format: PixelFormat,
	) -> Result<Buffer, Box<dyn Error>> {
		let id = self.wl_create_buffer((offset, width, height, stride), format)?;
		Ok(Buffer {
			id,
			ctx: self.ctx.clone(),
			offset,
			width,
			height,
			stride,
			format,
		})
	}

	fn wl_destroy(&self) -> Result<(), Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
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

	pub fn destroy(
		&self,
	) -> Result<(), Box<dyn Error>> {
		self.wl_destroy()?;
		self.ctx.borrow_mut().wlim.free_id(self.id)?;
		Ok(self.unlink()?)
	}
}

pub struct Buffer {
	pub id: Id,
	ctx: CtxType,
	pub offset: i32,
	pub width: i32,
	pub height: i32,
	pub stride: i32,
	pub format: PixelFormat,
}

impl Buffer {
	fn wl_destroy(&self) -> Result<(), Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 0,
			args: vec![],
		})
	}

	pub fn destroy(
		&self,
	) -> Result<(), Box<dyn Error>> {
		self.wl_destroy()?;
		self.ctx.borrow_mut().wlim.free_id(self.id)?;
		Ok(())
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

#[derive(Default, Debug)]
pub struct IdentManager {
	top_id: Id,
	free: Vec<Id>,
	idmap: HashMap<Id, WaylandObjectKind>,
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

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum PixelFormat {
	Argb888,
	Xrgb888,
}

impl PixelFormat {
	pub fn from_u32(processee: u32) -> Result<PixelFormat, Box<dyn Error>> {
		match processee {
			0 => Ok(PixelFormat::Argb888),
			1 => Ok(PixelFormat::Xrgb888),
			_ => Err(WaylandError::InvalidPixelFormat.boxed()),
		}
	}
}

pub struct XdgWmBase {
	pub id: Id,
	ctx: CtxType,
}

impl XdgWmBase {
	pub fn new_bound(
		display: &mut Display,
		registry: &mut Registry,
		ctx: CtxType,
	) -> Result<Self, Box<dyn Error>> {
		let id = ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::XdgWmBase);
		registry.wl_bind(id, WaylandObjectKind::XdgWmBase, 1)?;
		let cbid = display.wl_sync()?;

		let mut done = false;
		while !done {
			ctx.borrow_mut().wlmm.get_events()?;

			while let Some(msg) = ctx.borrow_mut().wlmm.q.pop_front() {
				if msg.recv_id == cbid {
					println!("xdg_wm_base callback done");
					done = true;
					break;
				}
			}
		}

		Ok(Self {
			id,
			ctx
		})
	}

	fn wl_destroy(&self) -> Result<(), Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 0,
			args: vec![],
		})
	}

	pub fn wl_pong(&self, serial: u32) -> Result<(), Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 3,
			args: vec![WireArgument::UnInt(serial)],
		})
	}

	pub fn destroy(
		&self,
	) -> Result<(), Box<dyn Error>> {
		self.wl_destroy()?;
		self.ctx.borrow_mut().wlim.free_id(self.id)?;
		Ok(())
	}

	fn wl_get_xdg_surface(
		&self,
		wl_surface_id: Id,
		xdg_surface_id: Id,
	) -> Result<u32, Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 2,
			args: vec![WireArgument::NewId(xdg_surface_id), WireArgument::Obj(wl_surface_id)],
		})?;
		Ok(xdg_surface_id)
	}

	pub fn make_xdg_surface(
		&self,
		wl_surface_id: Id,
	) -> Result<XdgSurface, Box<dyn Error>> {
		let xdg_surface_id = self.ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::XdgSurface);
		let id = self.wl_get_xdg_surface(wl_surface_id, xdg_surface_id)?;
		Ok(XdgSurface {
			id,
			ctx: self.ctx.clone(),
		})
	}
}

pub struct XdgSurface {
	pub id: Id,
	ctx: CtxType,
}

impl XdgSurface {
	fn wl_get_toplevel(
		&self,
		xdg_toplevel_id: Id,
	) -> Result<(), Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 1,
			args: vec![WireArgument::NewId(xdg_toplevel_id)],
		})
	}

	pub fn make_xdg_toplevel(
		&self,
	) -> Result<XdgTopLevel, Box<dyn Error>> {
		let id = self.ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::XdgTopLevel);
		self.wl_get_toplevel(id)?;
		Ok(XdgTopLevel {
			id,
			ctx: self.ctx.clone(),
		})
	}
}

pub struct XdgTopLevel {
	pub id: Id,
	ctx: CtxType,
}
