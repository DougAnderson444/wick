use std::collections::HashMap;
use std::sync::Arc;
use std::{env, fmt};

use async_trait::async_trait;
use config::{AppConfiguration, CliConfig, ImportBinding, TriggerDefinition};
// use futures::StreamExt;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use wick_packet::{packet_stream, Entity, Invocation};

use super::{resolve_ref, Trigger, TriggerKind};
use crate::dev::prelude::*;
use crate::resources::Resource;

#[derive(Debug)]
pub(crate) struct Cli {
  done_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
  done_rx: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Default)]
struct IsInteractive {
  stdin: bool,
  stdout: bool,
  stderr: bool,
}

impl Cli {
  pub(crate) fn load() -> Result<Arc<dyn Trigger + Send + Sync>, RuntimeError> {
    Ok(Arc::new(Cli::load_impl()?))
  }

  pub(crate) fn load_impl() -> Result<Cli, RuntimeError> {
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    Ok(Self {
      done_tx: Mutex::new(Some(done_tx)),
      done_rx: Mutex::new(Some(done_rx)),
    })
  }

  async fn handle_command(
    &self,
    app_config: AppConfiguration,
    config: CliConfig,
    args: Vec<String>,
  ) -> Result<(), RuntimeError> {
    let cli_component = resolve_ref(&app_config, config.component())?;
    let cli_binding = ImportBinding::component("cli", cli_component);

    let invocation = Invocation::new(
      Entity::server("cli_channel"),
      Entity::operation(&cli_binding.id, config.operation()),
      None,
    );

    let mut runtime = crate::RuntimeBuilder::new();
    runtime.add_import(cli_binding);
    for import in app_config.imports().values() {
      runtime.add_import(import.clone());
    }
    for resource in app_config.resources().values() {
      runtime.add_resource(resource.clone());
    }
    let runtime = runtime.build().await?;

    let is_interactive = IsInteractive {
      stdin: atty::is(atty::Stream::Stdin),
      stdout: atty::is(atty::Stream::Stdout),
      stderr: atty::is(atty::Stream::Stderr),
    };

    let packet_stream = packet_stream!(("args", args), ("isInteractive", is_interactive));

    let mut response = runtime.invoke(invocation, packet_stream, None).await?;
    while let Some(packet) = response.next().await {
      trace!(?packet, "trigger:cli:response");
    }

    let _ = self.done_tx.lock().take().unwrap().send(());

    Ok(())
  }
}

#[async_trait]
impl Trigger for Cli {
  async fn run(
    &self,
    name: String,
    app_config: AppConfiguration,
    config: TriggerDefinition,
    _resources: Arc<HashMap<String, Resource>>,
  ) -> Result<(), RuntimeError> {
    let config = if let TriggerDefinition::Cli(config) = config {
      config
    } else {
      return Err(RuntimeError::InvalidTriggerConfig(TriggerKind::Cli));
    };

    let mut args: Vec<String> = env::args().collect();
    // Preserve only the arguments after `--`.
    while !args.is_empty() && &args[0] != "--" {
      args.remove(0);
    }
    if !args.is_empty() && &args[0] == "--" {
      args.remove(0);
    }

    // Insert app name as the first argument.
    args.insert(0, name);

    self.handle_command(app_config, config, args).await?;

    Ok(())
  }

  async fn shutdown_gracefully(self) -> Result<(), RuntimeError> {
    Ok(())
  }

  async fn wait_for_done(&self) {
    let rx = self.done_rx.lock().take().unwrap();
    let _ = rx.await;
  }
}

impl fmt::Display for Cli {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Cli Trigger",)
  }
}
