use std::path::PathBuf;
use std::sync::PoisonError;

use itertools::join;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use vino_rpc::PortSignature;

use crate::dev::prelude::*;
use crate::schematic::PayloadReceived;
use crate::schematic_model::Connection;

type BoxedErrorSyncSend = Box<dyn std::error::Error + Sync + Send>;

#[derive(Error, Debug, PartialEq)]
pub enum ValidationError {
  #[error("Schematic '{0}' has errors: {}", join(.1, ", "))]
  PostInitError(String, Vec<ValidationError>),
  #[error("Schematic '{0}' has errors: {}", join(.1, ", "))]
  EarlyError(String, Vec<ValidationError>),
  #[error("Schematic has no outputs")]
  NoOutputs,
  #[error("Schematic has no inputs")]
  NoInputs,
  #[error("The following component(s) have incomplete internal model(s): '{}'", join(.0, ", "))]
  MissingComponentModels(Vec<String>),
  #[error("Dangling reference(s): '{}'", join(.0, ", "))]
  DanglingReference(Vec<String>),
  #[error("Component definition(s) '{}' not fully qualified", join(.0, ", "))]
  NotFullyQualified(Vec<String>),
  #[error("Invalid output port '{}' on {}. Valid output ports are [{}]", .0.name, .1, join(.2, ", "))]
  InvalidOutputPort(PortReference, Connection, Vec<PortSignature>),
  #[error("Invalid input port '{}' on {}. Valid input ports are [{}]", .0.name, .1, join(.2, ", "))]
  InvalidInputPort(PortReference, Connection, Vec<PortSignature>),
  #[error("Invalid connections: \n  {}", join(.0, "\n  "))]
  InvalidConnections(Vec<ValidationError>),
}

#[derive(Error, Debug, PartialEq)]
pub enum SchematicError {
  #[error("Schematic model not initialized")]
  ModelNotInitialized,
  #[error("Upstream port {0} not found")]
  UpstreamNotFound(PortReference),
  #[error("Transaction {0} not found")]
  TransactionNotFound(String),
  #[error("Reference {0} not found")]
  ReferenceNotFound(String),
  #[error("Schematic channel closed while data still available. This can happen when acting on output before waiting for the system to receive the final close and may not be a problem. Error: {0}")]
  SchematicClosedEarly(String),
}

#[derive(Error, Debug)]
pub enum NetworkError {
  #[error("Network not started")]
  NotStarted,
  #[error("Schematic {0} not found")]
  SchematicNotFound(String),
  #[error("Error initializing: {0}")]
  InitializationError(String),
  #[error(transparent)]
  ComponentError(#[from] ComponentError),
}

#[derive(Error, Debug)]
pub enum ComponentError {
  #[error("Could not extract claims from component")]
  ClaimsError,
  #[error(transparent)]
  WascapError(#[from] wascap::Error),
  #[error("Failed to create a raw WebAssembly host")]
  WapcError,
  #[error("File not found {}", .0.to_string_lossy())]
  FileNotFound(PathBuf),
  #[error(transparent)]
  ConversionError(#[from] ConversionError),
  #[error(transparent)]
  IOError(#[from] std::io::Error),
  #[error("Component not found, looked in {0}")]
  NotFound(String),
  #[error(transparent)]
  OciError(#[from] OciError),
  #[error(transparent)]
  ActixMailboxError(#[from] MailboxError),
  #[error(transparent)]
  RpcError(#[from] vino_rpc::Error),
  #[error(transparent)]
  OtherUpstream(#[from] BoxedErrorSyncSend),
  #[error(transparent)]
  RpcUpstreamError(#[from] tonic::Status),
  #[error(transparent)]
  OutputError(#[from] vino_component::Error),
  #[error(transparent)]
  CodecError(#[from] vino_codec::Error),
  #[error("Grpc Provider error: {0}")]
  GrpcUrlProviderError(String),
}

#[derive(Error, Debug)]
pub enum OciError {
  #[error("Configuration disallows fetching artifacts with the :latest tag ({0})")]
  LatestDisallowed(String),
  #[error("Could not fetch '{0}': {1}")]
  OciFetchFailure(String, String),
  #[error("Could not parse OCI URL {0}: {1}")]
  OCIParseError(String, String),
  #[error(transparent)]
  IOError(#[from] std::io::Error),
}

#[derive(Error, Debug, Clone, Copy)]
pub struct ConversionError(pub &'static str);

impl std::fmt::Display for ConversionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.0)
  }
}

#[derive(Error, Debug)]
pub enum VinoError {
  #[error("Conversion error {0}")]
  ConversionError(&'static str),
  #[error("URL parse error {0}")]
  ParseError(String),
  #[error(transparent)]
  ComponentError(#[from] ComponentError),
  #[error(transparent)]
  NetworkError(#[from] NetworkError),
  #[error("Error executing request: {0}")]
  ExecutionError(String),
  #[error("Schematic error: {0}")]
  SchematicError(String),
  #[error("Reference {0} not found")]
  ReferenceError(String),
  #[error("Dispatch error: {0}")]
  DispatchError(String),
  #[error("Provider error {0}")]
  ProviderError(String),
  #[error("WaPC WebAssembly Component error: {0}")]
  WapcError(String),
  #[error("Failed to acquire a lock: {0}")]
  LockError(String),
  #[error("Job error: {0}")]
  JobError(String),
  #[error("invalid configuration")]
  ConfigurationError,
  #[error("Could not start host: {0}")]
  HostStartFailure(String),
  #[error("Failed to deserialize configuration {0}")]
  ConfigurationDeserialization(String),
  #[error("Failed to serialize payload {0}")]
  SerializationError(rmp_serde::encode::Error),
  #[error("Failed to deserialize payload {0}")]
  DeserializationError(rmp_serde::decode::Error),
  #[error(transparent)]
  OciError(#[from] OciError),
  #[error(transparent)]
  SchematicErr(#[from] SchematicError),
  #[error(transparent)]
  TransactionChannelError(#[from] SendError<PayloadReceived>),
  #[error(transparent)]
  ValidationError(#[from] ValidationError),
  #[error(transparent)]
  TonicError(#[from] tonic::transport::Error),
  #[error(transparent)]
  RpcUpstreamError(#[from] tonic::Status),
  #[error(transparent)]
  EntityError(#[from] vino_entity::Error),
  #[error(transparent)]
  RpcError(#[from] vino_rpc::Error),
  #[error(transparent)]
  CodecError(#[from] vino_codec::Error),
  #[error(transparent)]
  ManifestError(#[from] vino_manifest::Error),
  #[error(transparent)]
  TransportError(#[from] vino_transport::Error),
  #[error(transparent)]
  OutputError(#[from] vino_component::Error),
  #[error(transparent)]
  ActixMailboxError(#[from] MailboxError),
  #[error(transparent)]
  IOError(#[from] std::io::Error),
  #[error(transparent)]
  KeyPairError(#[from] nkeys::error::Error),

  #[error(transparent)]
  OtherUpstream(#[from] BoxedErrorSyncSend),
  #[error("General error : {0}")]
  Other(String),
}

impl<T> From<PoisonError<std::sync::MutexGuard<'_, T>>> for VinoError {
  fn from(lock_error: PoisonError<std::sync::MutexGuard<'_, T>>) -> Self {
    VinoError::LockError(lock_error.to_string())
  }
}

impl From<&'static str> for VinoError {
  fn from(e: &'static str) -> Self {
    VinoError::Other(e.to_owned())
  }
}