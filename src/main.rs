use std::{cell::RefCell, env, error::Error, rc::Rc};

use wayland_raw::wayland::{
	Context, IdentManager,
	compositor::Compositor,
	display::Display,
	shm::{PixelFormat, SharedMemory},
	wire::MessageManager,
	xdgshell::{XdgTopLevel, XdgWmBase},
};

fn main() -> Result<(), Box<dyn Error>> {
	let wlim = IdentManager::default();
	let wlmm = MessageManager::new(&env::var("WAYLAND_DISPLAY")?)?;
	let ctx = Context::new(wlmm, wlim);
	let ctx = Rc::new(RefCell::new(ctx));

	let display = Display::new(ctx.clone());
	let registry = display.borrow_mut().make_registry()?;
	ctx.borrow_mut().handle_events()?;
	let compositor = Compositor::new_bound(&mut registry.borrow_mut(), ctx.clone())?;
	let surface = compositor.borrow_mut().make_surface()?;
	let shm = SharedMemory::new_bound_initialized(&mut registry.borrow_mut(), ctx.clone())?;
	let shm_pool = shm.borrow_mut().make_pool(500 * 500 * 4)?;
	unsafe {
		let x = &mut *shm_pool.borrow_mut().slice.unwrap();
		x.chunks_mut(4).for_each(|y| {
			y[0] = 255;
		});
	}
	ctx.borrow_mut().handle_events()?;
	let buf = shm_pool.borrow_mut().make_buffer((0, 500, 500, 500), PixelFormat::Xrgb888)?;
	let xdg_wm_base = XdgWmBase::new_bound(&mut registry.borrow_mut())?;
	let xdg_surface = xdg_wm_base.borrow_mut().make_xdg_surface(surface.clone())?;
	let xdg_toplevel = XdgTopLevel::new_from_xdg_surface(xdg_surface.clone())?;
	xdg_toplevel.borrow_mut().set_app_id(String::from("wayland-raw"))?;
	xdg_toplevel.borrow_mut().set_title(String::from("wayland-raw"))?;
	surface.borrow_mut().attach_buffer(buf.clone())?;
	surface.borrow_mut().commit()?;
	loop {
		if xdg_surface.borrow().is_configured {
			xdg_surface.borrow_mut().ack_configure()?;
			ctx.borrow_mut().handle_events()?;
			break;
		} else {
			ctx.borrow_mut().handle_events()?;
		}
	}
	Ok(())
}
