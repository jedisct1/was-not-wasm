use super::config::*;
use std::collections::HashMap;
use wasmer_runtime::Ctx;

#[derive(Debug)]
pub struct Allocation {
    pub offset: u32,
    pub start: u32,
    pub size: u32,
    pub rounded_size: u32,
}

#[derive(Debug)]
pub struct CtxData {
    pub heap_offset: u32,
    pub page_size: u32,
    pub allocations: HashMap<u32, Allocation>,
    pub canary: u8,
    pub junk: u8,
    pub canary_check_on_alloc: bool,
    pub alloc_count: u64,
    pub free_count: u64,
    pub alloc_total_usage: u64,
}

impl CtxData {
    pub fn new(runtime_config: RuntimeConfig) -> Self {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u32;
        let page_mask = page_size - 1;
        let heap_offset = (runtime_config.heap_base + page_mask) & !page_mask;
        let allocations = HashMap::new();
        let canary = 0xd0;
        let junk = 0xdb;
        CtxData {
            page_size,
            heap_offset,
            allocations,
            canary,
            junk,
            canary_check_on_alloc: runtime_config.canary_check_on_alloc,
            alloc_count: 0,
            free_count: 0,
            alloc_total_usage: 0,
        }
    }
}

pub fn get_or_create_ctx_data(ctx: &mut Ctx) -> Box<CtxData> {
    if ctx.data.is_null() {
        let ctx_data = Box::new(CtxData::new(RuntimeConfig::current()));
        ctx.data = Box::into_raw(ctx_data) as *mut _;
    }
    unsafe { Box::from_raw(ctx.data as *mut CtxData) }
}
