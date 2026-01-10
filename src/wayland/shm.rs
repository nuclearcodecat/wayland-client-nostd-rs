use std::{
	collections::HashSet, error::Error, ffi::CString, os::fd::RawFd
};
// std depends on libc anyway so i consider using it fair
// i may replace this with asm in the future but that means amd64 only
use libc::{O_CREAT, O_RDWR, ftruncate, shm_open, shm_unlink};
use crate::wayland::{CtxType, WaylandError, WaylandObjectKind, buffer::Buffer, display::Display, registry::Registry, wire::{Id, WireArgument, WireRequest}};

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

	pub(crate) fn wl_create_pool(
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

	pub(crate) fn wl_create_buffer(
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

	pub fn destroy(
		&self,
	) -> Result<(), Box<dyn Error>> {
		self.wl_destroy()?;
		self.ctx.borrow_mut().wlim.free_id(self.id)?;
		Ok(self.unlink()?)
	}
}
