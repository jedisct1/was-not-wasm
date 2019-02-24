extern crate structopt;

use libc;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, prelude::*};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::{mem, ptr};
use structopt::StructOpt;
use wasmer_runtime::{imports, instantiate, Ctx};

static HEAP_BASE: AtomicU32 = AtomicU32::new(0);
static CANARY_CHECK_ON_ALLOC: AtomicBool = AtomicBool::new(false);

#[derive(StructOpt, Debug)]
#[structopt(name = "was")]
struct Config {
    #[structopt(short = "f", long = "file", parse(from_os_str))]
    file: PathBuf,

    #[structopt(short = "b", long = "heap-base", default_value = "65536")]
    heap_base: u32,

    #[structopt(short = "c", long = "canary-check-on-alloc")]
    canary_check_on_alloc: bool,
}

#[derive(Debug)]
struct Allocation {
    offset: u32,
    start: u32,
    size: u32,
    rounded_size: u32,
}

#[derive(Debug)]
struct CtxData {
    heap_offset: u32,
    page_size: u32,
    allocations: HashMap<u32, Allocation>,
    canary: u8,
    canary_check_on_alloc: bool,
}

impl CtxData {
    fn new(heap_base: u32, canary_check_on_alloc: bool) -> Self {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u32;
        let page_mask = page_size - 1;
        let heap_offset = (heap_base + page_mask) & !page_mask;
        let allocations = HashMap::new();
        let canary = 0xd0;
        CtxData {
            page_size,
            heap_offset,
            allocations,
            canary,
            canary_check_on_alloc,
        }
    }
}

extern "C" fn debug_u64(val: u64, _ctx: &mut Ctx) {
    println!("Debug: [{}]", val);
}

extern "C" fn abort(_msg: u32, _file: u32, _line: u32, _col: u32, _ctx: &mut Ctx) {
    panic!("abort()");
}

extern "C" fn malloc(size: u32, ctx: &mut Ctx) -> u32 {
    if ctx.data.is_null() {
        let ctx_data = Box::new(CtxData::new(
            HEAP_BASE.load(Ordering::Relaxed),
            CANARY_CHECK_ON_ALLOC.load(Ordering::Relaxed),
        ));
        ctx.data = Box::into_raw(ctx_data) as *mut _;
    }
    let mut ctx_data = unsafe { Box::from_raw(ctx.data as *mut CtxData) };
    if ctx_data.canary_check_on_alloc {
        canaries_check(ctx);
    }
    let offset = ctx_data.heap_offset;
    let page_size = ctx_data.page_size;
    let page_mask = page_size - 1;
    let rounded_size = (size + page_mask) & !page_mask;
    let end = offset + rounded_size;
    let start = end - size;
    let heap_ptr = ctx.memory_mut(0).as_mut_ptr();
    unsafe {
        libc::mprotect(
            heap_ptr.offset(offset as isize) as *mut _,
            rounded_size as usize,
            libc::PROT_READ | libc::PROT_WRITE,
        )
    };
    if offset != start {
        unsafe {
            ptr::write_bytes(
                heap_ptr.offset(offset as isize),
                ctx_data.canary,
                (rounded_size - size) as usize,
            )
        };
    }
    ctx_data.allocations.insert(
        start,
        Allocation {
            offset,
            start,
            size,
            rounded_size,
        },
    );
    ctx_data.heap_offset = end + page_size;
    mem::forget(ctx_data);
    start
}

fn canary_check(allocation: &Allocation, ctx: &Ctx) {
    let ctx_data = unsafe { Box::from_raw(ctx.data as *mut CtxData) };
    let heap_ptr = ctx.memory(0).as_ptr();
    let canary = ctx_data.canary;
    for offset in allocation.offset..(allocation.offset + allocation.rounded_size - allocation.size)
    {
        if unsafe { *heap_ptr.offset(offset as isize) } != canary {
            panic!(
                "Corruption detected at offset {} (base: {})",
                offset, allocation.offset
            );
        }
    }
    mem::forget(ctx_data);
}

fn canaries_check(ctx: &Ctx) {
    let ctx_data = unsafe { Box::from_raw(ctx.data as *mut CtxData) };
    for allocation in ctx_data.allocations.values() {
        canary_check(allocation, ctx);
    }
    mem::forget(ctx_data);
}

extern "C" fn free(start: u32, ctx: &mut Ctx) {
    let mut ctx_data = unsafe { Box::from_raw(ctx.data as *mut CtxData) };
    let allocation = match ctx_data.allocations.get(&start) {
        None => panic!("free()ing invalid offset {}", start),
        Some(allocation) => allocation,
    };
    canary_check(&allocation, ctx);
    let heap_ptr = ctx.memory(0).as_ptr();
    unsafe {
        libc::mprotect(
            heap_ptr.offset(allocation.offset as isize) as *mut _,
            allocation.rounded_size as usize,
            libc::PROT_NONE,
        )
    };
    ctx_data.allocations.remove(&start);
    mem::forget(ctx_data);
}

fn main() -> Result<(), io::Error> {
    let config = Config::from_args();
    HEAP_BASE.store(config.heap_base, Ordering::Relaxed);
    CANARY_CHECK_ON_ALLOC.store(config.canary_check_on_alloc, Ordering::Relaxed);
    let import_object = imports! {
        "index" => {
            "puts" => puts<[u32] -> []>,
        },
        "env" => {
            "abort" => abort<[u32, u32, u32, u32] -> []>,
        },
        "system" => {
            "malloc" => malloc<[u32] -> [u32]>,
            "free" => free<[u32] -> []>,
        },
    };
    let mut file = File::open(config.file)?;
    let mut wasm = vec![];
    file.read_to_end(&mut wasm)?;
    let mut instance = instantiate(&wasm, import_object).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Unable to instantiate the module",
        )
    })?;
    let ctx = instance.context_mut();
    if ctx.data.is_null() {
        let ctx_data = Box::new(CtxData::new(
            HEAP_BASE.load(Ordering::Relaxed),
            CANARY_CHECK_ON_ALLOC.load(Ordering::Relaxed),
        ));
        ctx.data = Box::into_raw(ctx_data) as *mut _;
    }
    let mut ctx_data = unsafe { Box::from_raw(ctx.data as *mut CtxData) };
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
    instance.call("main", &[]).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Unable to run the webassembly code",
        )
    })?;
    Ok(())
}
