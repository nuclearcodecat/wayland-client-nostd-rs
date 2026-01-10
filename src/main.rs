#![allow(dead_code)]
#![feature(unix_socket_ancillary_data)]

use std::{cell::RefCell, env, error::Error, rc::Rc};

mod wayland;

use crate::wayland::{
	Compositor, Context, Display, IdentManager, Registry, SharedMemory, XdgWmBase, wire::{MessageManager, WireArgument}
};

fn main() -> Result<(), Box<dyn Error>> {
	let mut wlim = IdentManager::default();
	let mut wlmm = MessageManager::new(&env::var("WAYLAND_DISPLAY")?)?;
	let ctx = Context::new(wlmm, wlim);
	let ctx = Rc::new(RefCell::new(ctx));

	let mut display = Display::new(ctx.clone());
	let mut registry = Registry::new_filled(&mut display, ctx.clone())?;
	let compositor = Compositor::new_bound(&mut registry, ctx.clone())?;
	let mut surface = compositor.make_surface()?;
	let mut shm =
		SharedMemory::new_bound_initialized(&mut display, &mut registry, ctx.clone())?;
	let mut shm_pool = shm.make_pool(500 * 500 * 4)?;
	let buf = shm_pool.make_buffer(
		(0, 500, 500, 500),
		wayland::PixelFormat::Xrgb888,
	)?;
	let xdg_wm_base = XdgWmBase::new_bound(&mut display, &mut registry, ctx.clone())?;
	let xdg_surface = xdg_wm_base.make_xdg_surface(surface.id)?;
	let xdg_toplevel = xdg_surface.make_xdg_toplevel()?;
	surface.attach_buffer(buf.id)?;
	surface.commit()?;
	println!("hello");

	display.wl_sync()?;

	// wait for ping
	let mut ponged = false;
	while !ponged {
		wlmm.get_events()?;
		while let Some(ev) = wlmm.q.pop_front() {
			if ev.recv_id == xdg_wm_base.id
				&& ev.opcode == 0
				&& let WireArgument::UnInt(serial) = ev.args[0]
			{
				xdg_wm_base.wl_pong(serial)?;
				ponged = true;
				break;
			} else {
				println!("{:#?}", ev);
			}
		}
	}

	// USE INTERMUT SO SHIT DROPS WHEN PANICKING
	xdg_wm_base.destroy()?;
	buf.destroy()?;
	shm_pool.destroy()?;
	Ok(())
}
