use super::canary::*;
use super::ctx_data::*;
use std::{mem, ptr};
use wasmer_runtime::{imports, Ctx, ImportObject};

pub extern "C" fn debug_val(val: u32, _ctx: &mut Ctx) {
    println!("Debug: [{}]", val);
}

pub extern "C" fn abort(_msg: u32, _file: u32, _line: u32, _col: u32, _ctx: &mut Ctx) {
    panic!("abort()");
}

pub extern "C" fn malloc(size: u32, ctx: &mut Ctx) -> u32 {
    let mut ctx_data = get_or_create_ctx_data(ctx);
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
        );
        ptr::write_bytes(
            heap_ptr.offset(start as isize),
            ctx_data.junk,
            size as usize,
        );
    }
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
    ctx_data.alloc_count += 1;
    ctx_data.alloc_total_usage += u64::from(size);
    mem::forget(ctx_data);
    start
}

pub extern "C" fn free(start: u32, ctx: &mut Ctx) {
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
    ctx_data.free_count += 1;
    if ctx_data.free_count > ctx_data.alloc_count {
        panic!("free()ing unallocated memory");
    }
    mem::forget(ctx_data);
}

pub extern "C" fn terminate(ctx: &mut Ctx) {
    canaries_check(&ctx);
    let ctx_data = unsafe { Box::from_raw(ctx.data as *mut CtxData) };
    let leaked = ctx_data.alloc_count - ctx_data.free_count;
    eprintln!("Allocations:  {}", ctx_data.alloc_count);
    eprintln!("Leaked:       {}", leaked);
    eprintln!("Memory usage: {} bytes", ctx_data.alloc_total_usage);
    mem::forget(ctx_data);
    let heap = ctx.memory_mut(0);
    unsafe {
        libc::mprotect(
            heap.as_mut_ptr().offset(0) as *mut _,
            heap.len(),
            libc::PROT_READ | libc::PROT_WRITE,
        );
    }
}

pub fn import_object() -> ImportObject {
    imports! {
        "index" => {
            "debug_val" => debug_val<[u32] -> []>,
            "terminate" => terminate<[] -> []>,
        },
        "env" => {
            "abort" => abort<[u32, u32, u32, u32] -> []>,
        },
        "system" => {
            "malloc" => malloc<[u32] -> [u32]>,
            "free" => free<[u32] -> []>,
        },
    }
}
