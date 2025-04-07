use crate::ROOT_MOD;
use crate::bridge::Bridge;
use crate::handler::RubyHandler;
use magnus::{Class, Error, Module, Ruby};
use prosody::high_level::HighLevelClient;

mod config;

#[derive(Debug)]
#[magnus::wrap(class = "Prosody::NativeClient", free_immediately)]
pub struct NativeClient {
    client: HighLevelClient<RubyHandler>,
    bridge: Bridge,
}

impl NativeClient {
    fn initialize(ruby: &Ruby, this: &Self) {}
}

pub fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.get_inner(&ROOT_MOD);
    config::init(ruby)?;
    let class = module.define_class("NativeClient", ruby.class_object())?;

    // class.define_alloc_func::<NativeClient>();
    // class.define_method("initialize", method!(NativeClient::initialize, 0))?;

    Ok(())
}
