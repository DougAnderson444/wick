use std::path::Path;

use crate::dev::prelude::*;
pub use crate::providers::network_provider::Provider as NetworkProvider;
use crate::providers::wapc_module::WapcModule;

pub fn load_wasm_from_file(path: &Path) -> Result<WapcModule, CommonError> {
  WapcModule::from_file(path)
}

pub async fn load_wasm_from_oci(
  actor_ref: &str,
  allow_latest: bool,
  allowed_insecure: Vec<String>,
) -> Result<WapcModule, ComponentError> {
  let actor_bytes =
    crate::utils::oci::fetch_oci_bytes(actor_ref, allow_latest, &allowed_insecure).await?;
  Ok(WapcModule::from_slice(&actor_bytes)?)
}

pub async fn load_wasm(
  actor_ref: &str,
  allow_latest: bool,
  allowed_insecure: Vec<String>,
) -> Result<WapcModule, ComponentError> {
  let path = Path::new(&actor_ref);
  if path.exists() {
    Ok(WapcModule::from_file(path)?)
  } else {
    load_wasm_from_oci(actor_ref, allow_latest, allowed_insecure).await
  }
}

pub(crate) fn keypair_from_seed(seed: &str) -> Result<KeyPair, CommonError> {
  KeyPair::from_seed(seed).map_err(|_| CommonError::KeyPairFailed)
}