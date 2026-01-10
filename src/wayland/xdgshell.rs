use std::error::Error;

use crate::wayland::{CtxType, WaylandObjectKind, display::Display, registry::Registry, wire::{Id, WireArgument, WireRequest}};

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

	pub(crate) fn wl_destroy(&self) -> Result<(), Box<dyn Error>> {
		self.ctx.borrow().wlmm.send_request(&mut WireRequest {
			sender_id: self.id,
			opcode: 0,
			args: vec![],
		})
	}

	pub(crate) fn wl_pong(&self, serial: u32) -> Result<(), Box<dyn Error>> {
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

	pub(crate) fn wl_get_xdg_surface(
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
	pub(crate) fn wl_get_toplevel(
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
