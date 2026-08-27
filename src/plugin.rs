use crate::{
    equalizer::{Equalizer, RetryPolicy},
    herdr::{env::PluginEnv, lock::StateLock, socket::SocketClient, trigger::Trigger},
};
use anyhow::Result;
use std::{num::NonZeroU32, time::Duration};

const RETRY: RetryPolicy = RetryPolicy {
    attempts: NonZeroU32::new(20).unwrap(),
    interval: Duration::from_millis(5),
};

pub fn run() -> Result<()> {
    let env = PluginEnv::from_process()?;
    let trigger = Trigger::from_env(&env)?;

    // NOTE: another instance rearranges the tabs while we wait unless we lock first
    let _lock = StateLock::acquire(&env.state_dir)?;

    Equalizer::new(SocketClient::new(&env.socket_path), RETRY).run(&trigger)
}
