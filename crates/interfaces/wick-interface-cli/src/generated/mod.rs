pub use wick_component::{packet as wick_packet, wasmrs, wasmrs_codec};
#[allow(unused)]
pub(crate) type WickStream<T> = wick_component::wasmrs_rx::BoxFlux<T, wick_component::anyhow::Error>;
pub use wick_component::anyhow::Result;
pub use wick_component::flow_component::Context;
pub mod types {
  #[allow(unused)]
  use super::types;
  #[derive(Debug, Clone, serde :: Serialize, serde :: Deserialize, PartialEq)]
  pub struct Interactive {
    pub stdin: bool,
    pub stdout: bool,
    pub stderr: bool,
  }
}
#[derive(Default, Clone)]
pub struct Component;
impl Component {}