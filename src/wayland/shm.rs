use std::{cell::RefCell, collections::HashSet, error::Error, ffi::CString, os::{fd::RawFd, raw::c_void}, ptr::{self, null_mut}, rc::Rc};
// std depends on libc anyway so i consider using it fair
// i may replace this with asm in the future but that means amd64 only
use crate::{
	drop,
	wayland::{
		CtxType, EventAction, RcCell, WaylandError, WaylandObject, WaylandObjectKind,
		buffer::Buffer,
		registry::Registry,
		wire::{FromWirePayload, Id, WireArgument, WireRequest},
	},
};
use libc::{MAP_FAILED, MAP_SHARED, O_CREAT, O_RDWR, PROT_READ, PROT_WRITE, ftruncate, mmap, munmap, shm_open, shm_unlink};

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
		registry: &mut Registry,
		ctx: CtxType,
	) -> Result<RcCell<Self>, Box<dyn Error>> {
		let shm = Rc::new(RefCell::new(Self::new(0, ctx.clone())));
		let id =
			ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::SharedMemory, shm.clone());
		shm.borrow_mut().id = id;
		registry.bind(id, WaylandObjectKind::SharedMemory, 1)?;
		Ok(shm)
	}

	pub fn make_pool(&mut self, size: i32) -> Result<RcCell<SharedMemoryPool>, Box<dyn Error>> {
		// add method to get new names
		let name = CString::new("wl-shm-1")?;
		let fd = unsafe { shm_open(name.as_ptr(), O_RDWR | O_CREAT, 0) };
		println!("fd: {}", fd);
		unsafe { ftruncate(fd, size.into()) };

		let shmpool =
			Rc::new(RefCell::new(SharedMemoryPool::new(0, self.ctx.clone(), name, fd, size)));
		let id = self
			.ctx
			.borrow_mut()
			.wlim
			.new_id_registered(WaylandObjectKind::SharedMemoryPool, shmpool.clone());
		shmpool.borrow_mut().id = id;

		let ptr = unsafe { mmap(null_mut(), size as usize, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0) };
		if ptr == MAP_FAILED {
			return Err(Box::new(std::io::Error::last_os_error()));
		} else {
			let x: *mut [u8] = ptr::slice_from_raw_parts_mut(ptr as *mut u8, size as usize);
			shmpool.borrow_mut().ptr = Some(ptr);
			shmpool.borrow_mut().slice = Some(x);
		};

		self.wl_create_pool(size, fd, id)?;
		Ok(shmpool)
	}

	pub(crate) fn wl_create_pool(
		&self,
		size: i32,
		fd: RawFd,
		id: Id,
	) -> Result<(), Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 0,
			args: vec![
				// WireArgument::NewIdSpecific(WaylandObjectKind::SharedMemoryPool.as_str(), 1, id),
				WireArgument::NewId(id),
				WireArgument::FileDescriptor(fd),
				WireArgument::Int(size),
			],
		})
	}
}

pub struct SharedMemoryPool {
	id: Id,
	ctx: CtxType,
	name: CString,
	pub size: i32,
	fd: RawFd,
	pub slice: Option<*mut [u8]>,
	ptr: Option<*mut c_void>,
}

impl SharedMemoryPool {
	pub fn new(id: Id, ctx: CtxType, name: CString, size: i32, fd: RawFd) -> Self {
		Self {
			id,
			ctx,
			name,
			size,
			fd,
			slice: None,
			ptr: None,
		}
	}

	pub(crate) fn wl_create_buffer(
		&self,
		id: Id,
		(offset, width, height, stride): (i32, i32, i32, i32),
		format: PixelFormat,
	) -> Result<(), Box<dyn Error>> {
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
		})
	}

	pub fn make_buffer(
		&self,
		(offset, width, height, stride): (i32, i32, i32, i32),
		format: PixelFormat,
	) -> Result<RcCell<Buffer>, Box<dyn Error>> {
		let buf = Rc::new(RefCell::new(Buffer {
			id: 0,
			ctx: self.ctx.clone(),
			offset,
			width,
			height,
			stride,
			format,
			in_use: false,
		}));
		let id =
			self.ctx.borrow_mut().wlim.new_id_registered(WaylandObjectKind::Buffer, buf.clone());
		buf.borrow_mut().id = id;
		self.wl_create_buffer(id, (offset, width, height, stride), format)?;
		Ok(buf)
	}

	pub(crate) fn unmap(&self) -> Result<(), std::io::Error> {
		let r = unsafe { munmap(self.ptr.unwrap(), self.size as usize) };
		if r == 0 {
			Ok(())
		} else {
			Err(std::io::Error::last_os_error())
		}
	}

	pub(crate) fn wl_destroy(&self) -> Result<(), Box<dyn Error>> {
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

	pub fn destroy(&self) -> Result<(), Box<dyn Error>> {
		self.wl_destroy()?;
		self.ctx.borrow_mut().wlim.free_id(self.id)?;
		self.unlink()?;
		self.unmap()?;
		Ok(())
	}
}

impl WaylandObject for SharedMemory {
	fn handle(
		&mut self,
		opcode: super::OpCode,
		payload: &[u8],
	) -> Result<Vec<EventAction>, Box<dyn Error>> {
		let mut pending = vec![];
		match opcode {
			0 => {
				let format = u32::from_wire(payload)?;
				if let Ok(pf) = PixelFormat::from_u32(format) {
					self.push_pix_format(pf);
					pending.push(EventAction::DebugMessage(
						crate::wayland::DebugLevel::Verbose,
						format!("pushing pixel format {:?} (0x{:08x})", pf, format),
					));
				} else {
					pending.push(EventAction::DebugMessage(
						crate::wayland::DebugLevel::Important,
						format!("found unrecognized pixel format 0x{:08x}", format),
					));
				}
			}
			inv => {
				return Err(WaylandError::InvalidOpCode(inv, self.as_str()).boxed());
			}
		}
		Ok(pending)
	}

	fn as_str(&self) -> &'static str {
		WaylandObjectKind::SharedMemory.as_str()
	}
}

impl WaylandObject for SharedMemoryPool {
	fn handle(
		&mut self,
		opcode: super::OpCode,
		payload: &[u8],
	) -> Result<Vec<EventAction>, Box<dyn Error>> {
		todo!()
	}

	fn as_str(&self) -> &'static str {
		WaylandObjectKind::SharedMemoryPool.as_str()
	}
}

drop!(SharedMemoryPool);
