use std::
	error::Error
;
use crate::wayland::{CtxType, OpCode, WaylandObject, WaylandObjectKind, wire::{Id, WireArgument, WireArgumentKind, WireRequest, decode_event_payload}};

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

	pub(super) fn wl_get_registry(
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

	pub(super) fn wl_sync(
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

impl WaylandObject for Display {
	fn handle(&mut self, opcode: OpCode, payload: &[u8]) -> Result<(), Box<dyn Error>> {
		match opcode {
			1 => {
				let deleted_id =
					decode_event_payload(&payload[8..], WireArgumentKind::UnInt)?;
				println!(
					"==================== ID {:?} GOT DELETED (unimpl)",
					deleted_id
				);
				if let WireArgument::UnInt(did) = deleted_id {
					self.ctx.borrow_mut().wlim.free_id(did);
				}
			}
			_ => {
				eprintln!("unimplemented display event");
			}
		}
		Ok(())
	}
}
