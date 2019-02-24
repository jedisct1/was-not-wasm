extern crate structopt;

mod canary;
mod config;
mod ctx_data;
mod imports;

use config::*;
use ctx_data::*;
use imports::*;
use libc;
use std::fs::File;
use std::io::{self, prelude::*};
use wasmer_runtime::instantiate;

fn main() -> Result<(), io::Error> {
    let config = Config::parse();

    let mut wasm = vec![];
    File::open(config.file)?.read_to_end(&mut wasm)?;
    let mut instance = instantiate(&wasm, import_object()).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unable to instantiate the module: {:?}", e),
        )
    })?;
    let ctx = instance.context_mut();
    let mut ctx_data = get_or_create_ctx_data(ctx);
    let heap = ctx.memory_mut(0);
    unsafe {
        libc::mprotect(
            heap.as_mut_ptr().offset(0) as *mut _,
            ctx_data.heap_offset as usize,
            libc::PROT_READ,
        );
        libc::mprotect(
            heap.as_mut_ptr().offset(ctx_data.heap_offset as isize) as *mut _,
            heap.len() - ctx_data.heap_offset as usize,
            libc::PROT_NONE,
        );
    }
    ctx_data.heap_offset += ctx_data.page_size;
    instance.call(&config.entrypoint, &[]).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unable to run the webassembly code: {:?}", e),
        )
    })?;
    Ok(())
}
