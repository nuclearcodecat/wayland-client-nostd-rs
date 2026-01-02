#![allow(dead_code)]

use std::env;

mod wayland;

use crate::wayland::{Display, IdManager, wire::MessageManager, Registry};

fn main() -> Result<(), ()> {
	let display_name = env::var("WAYLAND_DISPLAY").map_err(|_| {})?;
	let mut wlim = IdManager::default();
	let mut wlmm = MessageManager::new(&display_name)?;
	let mut display = Display::new(&mut wlim);
	let reg_id = display.wl_get_registry(&mut wlmm, &mut wlim)?;
	let mut registry = Registry::new(reg_id);

	display.wl_sync(&mut wlmm, &mut wlim)?;
	registry.wl_bind(&mut wlmm, &mut wlim)?;

	let mut read = wlmm.get_events()?;
	while read.is_none() {
		read = wlmm.get_events()?;
	}
	// println!("\n\n==== EVENT\n{:#?}", read);
	let read = &read.unwrap();
	registry.fill(read)?;
	println!("\n\n==== REGISTRY\n{:#?}", registry.inner);

	wlmm.discon()?;
	println!("good");
	Ok(())
}
