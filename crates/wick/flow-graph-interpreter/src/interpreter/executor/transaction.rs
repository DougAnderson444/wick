use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use flow_component::{RuntimeCallback, SharedComponent};
use flow_graph::{NodeIndex, PortReference, SCHEMATIC_OUTPUT_INDEX};
use futures::StreamExt;
use parking_lot::Mutex;
use seeded_random::{Random, Seed};
use uuid::Uuid;
use wasmrs_rx::Observer;
use wick_packet::{Invocation, Packet, PacketError, PacketSender, PacketStream};

use self::operation::InstanceHandler;
use super::error::ExecutionError;
use crate::graph::types::*;
use crate::graph::LiquidOperationConfig;
use crate::interpreter::channel::InterpreterDispatchChannel;
use crate::interpreter::error::StateError;
use crate::interpreter::executor::transaction::operation::port::PortStatus;
use crate::{HandlerMap, InterpreterOptions};

pub(crate) mod operation;

pub(crate) mod statistics;
pub(crate) use statistics::TransactionStatistics;

type Result<T> = std::result::Result<T, ExecutionError>;

#[derive(Debug, Clone, Copy)]
pub(crate) enum TxState {
  OutputPending,
  OutputComplete,
}

#[derive()]
#[must_use]
pub struct Transaction {
  schematic: Arc<Schematic>,
  output: (Option<PacketSender>, Option<PacketStream>),
  channel: InterpreterDispatchChannel,
  invocation: Invocation,
  instances: Vec<Arc<InstanceHandler>>,
  id: Uuid,
  start_time: Instant,
  finished: AtomicBool,
  span: tracing::Span,
  callback: Arc<RuntimeCallback>,
  config: LiquidOperationConfig,
  pub(crate) last_access_time: Mutex<SystemTime>,
  pub(crate) stats: TransactionStatistics,
}

impl std::fmt::Debug for Transaction {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Transaction").field("id", &self.id).finish()
  }
}

#[allow(clippy::too_many_arguments)]
impl Transaction {
  pub(crate) fn new(
    schematic: Arc<Schematic>,
    mut invocation: Invocation,
    channel: InterpreterDispatchChannel,
    components: &Arc<HandlerMap>,
    self_component: &SharedComponent,
    callback: Arc<RuntimeCallback>,
    config: LiquidOperationConfig,
    seed: Seed,
  ) -> Self {
    let instances: Vec<_> = schematic
      .nodes()
      .iter()
      .map(|component| {
        Arc::new(InstanceHandler::new(
          schematic.clone(),
          component,
          components.clone(),
          self_component.clone(),
        ))
      })
      .collect();

    let rng = Random::from_seed(seed);
    let id = rng.uuid();
    invocation.tx_id = id;
    let stats = TransactionStatistics::new(id);
    stats.mark("new");
    let span = invocation.following_span(trace_span!("tx",tx_id=%id));

    let (tx, rx) = invocation.make_response();

    Self {
      channel,
      invocation,
      schematic,
      config,
      output: (Some(tx), Some(rx)),
      instances,
      start_time: Instant::now(),
      stats,
      last_access_time: Mutex::new(SystemTime::now()),
      id,
      span,
      finished: AtomicBool::new(false),
      callback,
    }
  }

  pub fn id(&self) -> Uuid {
    self.id
  }

  pub fn schematic_name(&self) -> &str {
    self.schematic.name()
  }

  pub(crate) fn schematic(&self) -> &Schematic {
    &self.schematic
  }

  pub(crate) fn output_handler(&self) -> &InstanceHandler {
    &self.instances[self.schematic.output().index()]
  }

  pub(crate) fn instance(&self, index: NodeIndex) -> &Arc<InstanceHandler> {
    &self.instances[index]
  }

  pub(crate) fn done(&self) -> bool {
    let output_handler = self.output_handler();
    let outputs_done = output_handler
      .inputs()
      .iter()
      .all(|p| p.status() == PortStatus::DoneClosed && p.is_empty());

    outputs_done
  }

  pub(crate) async fn start(&mut self, options: &InterpreterOptions) -> Result<()> {
    self.stats.mark("start");
    self.stats.start("execution");
    self.span.in_scope(|| trace!("starting transaction"));

    self.start_time = Instant::now();

    for instance in self.instances.iter() {
      if instance.index() == SCHEMATIC_OUTPUT_INDEX {
        continue;
      }
      let invocation = self
        .invocation
        .next_tx(self.invocation.origin.clone(), instance.entity());
      instance
        .clone()
        .start(
          self.id(),
          invocation,
          self.channel.clone(),
          options,
          self.callback.clone(),
          self.config.clone(),
        )
        .await?;
    }

    let incoming = self.invocation.eject_stream();

    self.prime_input_ports(self.schematic.input().index(), incoming)?;

    self.stats.mark("start_done");
    Ok(())
  }

  fn prime_input_ports(&self, index: NodeIndex, mut payloads: PacketStream) -> Result<()> {
    let input = self.instance(index).clone();
    let channel = self.channel.clone();
    let tx_id = self.id();

    tokio::spawn(async move {
      while let Some(Ok(packet)) = payloads.next().await {
        if let Ok(port) = input.find_input(packet.port()) {
          accept_input(tx_id, port, &input, &channel, packet).await;
        } else if packet.is_noop() {
          // TODO: propagate this and/or its context if it becomes an issue.
        } else {
          warn!(port = packet.port(), "dropping packet for unconnected port");
        }
      }
    });
    Ok(())
  }

  pub(crate) fn update_last_access(&self) {
    let now = SystemTime::now();
    let elapsed = now.duration_since(self.last_access());
    trace!(?elapsed, "updating last access");
    *self.last_access_time.lock() = now;
  }

  pub(crate) fn last_access(&self) -> SystemTime {
    *self.last_access_time.lock()
  }

  // Run when the transaction has finished delivering output to its output ports.
  //
  // A transaction may still be executing operations with side effects after this point.
  pub(crate) fn finish(&mut self) -> Result<&TransactionStatistics> {
    trace!("finishing transaction core");

    // drop our output sender;
    drop(self.output.0.take());

    // mark our end of execution
    self.stats.end("execution");

    // print stats if we're in tests.
    #[cfg(test)]
    self.stats.print();

    Ok(&self.stats)
  }

  pub(crate) async fn emit_output_message(&self, packets: Vec<Packet>) -> Result<()> {
    if let Some(ref output) = self.output.0 {
      for packet in packets {
        output.send(packet).map_err(|_e| ExecutionError::ChannelSend)?;
      }
    } else if packets.iter().any(|p| !p.is_done()) {
      error!(tx_id = %self.id(), "attempted to send output message after tx finished");
    }

    if self.done() {
      self.emit_done().await?;
    }
    Ok(())
  }

  pub(crate) async fn emit_done(&self) -> Result<()> {
    if !self.finished.load(Ordering::Relaxed) {
      self.span.in_scope(|| trace!("tx finished, dispatching done"));
      self.finished.store(true, Ordering::Relaxed);
      self.channel.dispatch_done(self.id()).await;
    }
    Ok(())
  }

  pub(crate) fn take_stream(&mut self) -> Option<PacketStream> {
    self.output.1.take()
  }

  pub(crate) fn take_tx_output(&self) -> Result<Vec<Packet>> {
    let output = self.output_handler();
    output
      .take_packets()
      .map_err(|_| ExecutionError::InvalidState(StateError::PayloadMissing(output.id().to_owned())))
  }

  pub(crate) fn _take_packets(&self, instance: &InstanceHandler) -> Result<Vec<Packet>> {
    instance.take_packets()
  }

  pub(crate) fn take_component_output(&self, port: &PortReference) -> Option<Packet> {
    let instance = self.instance(port.node_index());
    instance.take_output(port)
  }

  pub(crate) fn take_component_input(&self, port: &PortReference) -> Option<Packet> {
    let instance = self.instance(port.node_index());
    instance.take_input(port)
  }

  pub(crate) async fn check_stalled(&self) -> Result<TxState> {
    if self.done() {
      Ok(TxState::OutputComplete)
    } else {
      warn!(tx_id = %self.id(), "transaction hung");
      self
        .emit_output_message(vec![Packet::component_error("Transaction hung")])
        .await?;
      Ok(TxState::OutputPending)
    }
  }

  pub(crate) fn push_packets(&self, index: NodeIndex, packets: Vec<Packet>) -> Result<()> {
    let instance = self.instance(index).clone();

    let _ = instance.accept_packets(packets);

    Ok(())
  }

  pub(crate) async fn handle_schematic_output(&self) -> Result<()> {
    self.emit_output_message(self.take_tx_output()?).await?;

    Ok(())
  }

  pub(crate) async fn handle_op_err(&self, index: NodeIndex, err: PacketError) -> Result<()> {
    self.stats.mark(format!("component:{}:op_err", index));
    let instance = self.instance(index);

    let graph = self.schematic();

    for port in instance.outputs().refs() {
      let downport_name = graph.get_port_name(&port);
      let down_instance = self.instance(port.node_index());
      accept_outputs(
        self.id(),
        port,
        down_instance.clone(),
        self.channel.clone(),
        vec![Packet::raw_err(downport_name, err.clone()), Packet::done(downport_name)],
      )
      .await;
    }
    Ok(())
  }
}

pub(crate) async fn accept_input<'a, 'b>(
  tx_id: Uuid,
  port: PortReference,
  instance: &'a Arc<InstanceHandler>,
  channel: &'b InterpreterDispatchChannel,
  payload: Packet,
) {
  instance.buffer_in(&port, payload);
  channel.dispatch_data(tx_id, port).await;
}

pub(crate) async fn accept_outputs(
  tx_id: Uuid,
  port: PortReference,
  instance: Arc<InstanceHandler>,
  channel: InterpreterDispatchChannel,
  msgs: Vec<Packet>,
) {
  for payload in msgs {
    accept_output(tx_id, port, &instance, &channel, payload).await;
  }
}
pub(crate) async fn accept_output<'a, 'b>(
  tx_id: Uuid,
  port: PortReference,
  instance: &'a Arc<InstanceHandler>,
  channel: &'b InterpreterDispatchChannel,
  payload: Packet,
) {
  instance.buffer_out(&port, payload);
  channel.dispatch_data(tx_id, port).await;
}
