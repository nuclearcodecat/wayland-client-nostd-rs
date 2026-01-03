#![allow(dead_code)]

use std::{env, error::Error};

mod wayland;

use crate::wayland::{
	Compositor, Display, IdManager, Registry, SharedMemory, SharedMemoryPool, wire::MessageManager,
};

fn main() -> Result<(), Box<dyn Error>> {
	let mut wlim = IdManager::default();
	let mut wlmm = MessageManager::new(&env::var("WAYLAND_DISPLAY")?)?;

	let mut display = Display::new(&mut wlim);
	let mut registry = Registry::new_bound_filled(&mut display, &mut wlmm, &mut wlim)?;
	let compositor = Compositor::new_bound(&mut registry, &mut wlmm, &mut wlim)?;
	let mut shm = SharedMemory::new_bound(&mut registry, &mut wlmm, &mut wlim)?;
	let shm_pool = SharedMemoryPool::new_bound(&mut shm, 500 * 500 * 3, &mut wlmm, &mut wlim)?;

	// store an Rc to wlmm and wlim in every object? then i won't have to clean up and may just depend on Drop
	shm_pool.destroy(&mut wlmm, &mut wlim)?;
	Ok(())
}
