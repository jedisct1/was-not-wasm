use std::cell::RefCell;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Default, Debug, Copy, Clone)]
pub struct RuntimeConfig {
    pub heap_base: u32,
    pub canary_check_on_alloc: bool,
}

thread_local! {
    static RUNTIME_CONFIG: RefCell<RuntimeConfig> = RefCell::new(RuntimeConfig::default())
}

impl RuntimeConfig {
    pub fn current() -> Self {
        RUNTIME_CONFIG.with(|runtime_config| *runtime_config.borrow())
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "WAS (not WASM)")]
pub struct Config {
    #[structopt(short = "f", long = "file", parse(from_os_str))]
    pub file: PathBuf,

    #[structopt(short = "b", long = "heap-base", default_value = "65536")]
    pub heap_base: u32,

    #[structopt(short = "c", long = "canary-check-on-alloc")]
    pub canary_check_on_alloc: bool,

    #[structopt(short = "e", long = "entrypoint", default_value = "main")]
    pub entrypoint: String,
}

impl Config {
    pub fn parse() -> Self {
        let config = Config::from_args();
        RUNTIME_CONFIG.with(|runtime_config| {
            *runtime_config.borrow_mut() = RuntimeConfig {
                heap_base: config.heap_base,
                canary_check_on_alloc: config.canary_check_on_alloc,
            };
        });
        config
    }
}
