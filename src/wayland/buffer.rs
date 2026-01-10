use std::error::Error;

use crate::wayland::{CtxType, shm::PixelFormat, wire::{Id, WireRequest}};

pub struct Buffer {
	pub id: Id,
	pub(crate) ctx: CtxType,
	pub offset: i32,
	pub width: i32,
	pub height: i32,
	pub stride: i32,
	pub format: PixelFormat,
}

impl Buffer {
	pub(crate) fn wl_destroy(&self) -> Result<(), Box<dyn Error>> {
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

