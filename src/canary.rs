use super::ctx_data::*;
use std::mem;
use wasmer_runtime::Ctx;

pub fn canary_check(allocation: &Allocation, ctx: &Ctx) {
    let ctx_data = unsafe { Box::from_raw(ctx.data as *mut CtxData) };
    let heap_ptr = &ctx.memory(0)[allocation.offset as usize..].as_ptr();
    let canary = ctx_data.canary;
    for offset in 0..(allocation.rounded_size - allocation.size) {
        if unsafe { *heap_ptr.offset(offset as isize) } != canary {
            panic!(
                "Corruption detected at offset {} (base: {})",
                offset, allocation.offset
            );
        }
    }
    mem::forget(ctx_data);
}

pub fn canaries_check(ctx: &Ctx) {
    let ctx_data = unsafe { Box::from_raw(ctx.data as *mut CtxData) };
    for allocation in ctx_data.allocations.values() {
        canary_check(allocation, ctx);
    }
    mem::forget(ctx_data);
}
