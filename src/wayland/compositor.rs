use std::error::Error;

use crate::wayland::{CtxType, WaylandObjectKind, registry::Registry, surface::Surface, wire::{Id, WireArgument, WireRequest}};

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

