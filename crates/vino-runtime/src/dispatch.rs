use std::convert::TryFrom;
use std::fmt::Display;
use std::io::Read;

use actix::dev::MessageResponse;
use actix::prelude::Message;
use actix::Actor;
use data_encoding::HEXUPPER;
use ring::digest::{
  Context,
  Digest,
  SHA256,
};
use serde::{
  Deserialize,
  Serialize,
};
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;
use vino_transport::MessageTransport;
use wascap::prelude::{
  Claims,
  KeyPair,
};

use crate::error::VinoError;
use crate::schematic::PushOutput;
use crate::{
  Error,
  Result,
};

/// An invocation for a component, port, or schematic
#[derive(Debug, Clone, Default, Serialize, Deserialize, Message, PartialEq)]
#[rtype(result = "InvocationResponse")]
pub struct Invocation {
  pub origin: VinoEntity,
  pub target: VinoEntity,
  pub msg: MessageTransport,
  pub id: String,
  pub tx_id: String,
  pub encoded_claims: String,
  pub host_id: String,
}

impl<A, M> MessageResponse<A, M> for Invocation
where
  A: Actor,
  M: Message<Result = Invocation>,
{
  fn handle(self, _: &mut A::Context, tx: Option<actix::dev::OneshotSender<Self>>) {
    if let Some(tx) = tx {
      if let Err(e) = tx.send(self) {
        error!("Send error (call id:{} target:{:?})", &e.id, &e.target);
      }
    }
  }
}

impl TryFrom<Invocation> for vino_rpc::rpc::Invocation {
  type Error = VinoError;
  fn try_from(inv: Invocation) -> Result<Self> {
    Ok(vino_rpc::rpc::Invocation {
      origin: Some(inv.origin.into()),
      target: Some(inv.target.into()),
      msg: inv.msg.into_multibytes()?,
      id: inv.id,
      tx_id: inv.tx_id,
      encoded_claims: inv.encoded_claims,
      host_id: inv.host_id,
    })
  }
}

impl From<VinoEntity> for vino_rpc::rpc::Entity {
  fn from(ent: VinoEntity) -> Self {
    use vino_rpc::rpc::entity::EntityKind;
    use vino_rpc::rpc::Entity;
    match ent {
      VinoEntity::Test(v) => Entity {
        name: v,
        kind: EntityKind::Test.into(),
      },
      VinoEntity::Schematic(v) => Entity {
        name: v,
        kind: EntityKind::Schematic.into(),
      },
      VinoEntity::Component(v) => Entity {
        name: v.name,
        kind: EntityKind::Component.into(),
      },
      VinoEntity::Provider(v) => Entity {
        name: v,
        kind: EntityKind::Provider.into(),
      },
    }
  }
}

#[derive(Debug)]
pub enum InvocationResponse {
  Success {
    tx_id: String,
    msg: MessageTransport,
  },
  Stream {
    tx_id: String,
    rx: UnboundedReceiver<PushOutput>,
  },
  Error {
    tx_id: String,
    msg: String,
  },
}

impl InvocationResponse {
  /// Creates a successful invocation response stream. Response include the receiving end
  /// of an unbounded channel to listen for future output.
  pub fn stream(tx_id: String, rx: UnboundedReceiver<PushOutput>) -> InvocationResponse {
    InvocationResponse::Stream { tx_id, rx }
  }

  /// Creates a successful invocation response. Successful invocations include the payload for an
  /// invocation
  pub fn success(tx_id: String, msg: MessageTransport) -> InvocationResponse {
    InvocationResponse::Success { tx_id, msg }
  }

  /// Creates an error response
  pub fn error(tx_id: String, msg: String) -> InvocationResponse {
    InvocationResponse::Error { tx_id, msg }
  }

  pub fn tx_id(&self) -> &str {
    match self {
      InvocationResponse::Stream { tx_id, .. } => tx_id,
      InvocationResponse::Success { tx_id, .. } => tx_id,
      InvocationResponse::Error { tx_id, .. } => tx_id,
    }
  }

  pub fn to_stream(self) -> Result<(String, UnboundedReceiver<PushOutput>)> {
    match self {
      InvocationResponse::Stream { tx_id, rx } => Ok((tx_id, rx)),
      _ => Err(crate::Error::ConversionError("to_stream")),
    }
  }

  pub fn to_success(self) -> Result<(String, MessageTransport)> {
    match self {
      InvocationResponse::Success { tx_id, msg } => Ok((tx_id, msg)),
      _ => Err(crate::Error::ConversionError("to_success")),
    }
  }

  pub fn to_error(self) -> Result<(String, String)> {
    match self {
      InvocationResponse::Error { tx_id, msg } => Ok((tx_id, msg)),
      _ => Err(crate::Error::ConversionError("to_error")),
    }
  }
}

impl<A, M> MessageResponse<A, M> for InvocationResponse
where
  A: Actor,
  M: Message<Result = InvocationResponse>,
{
  fn handle(self, _: &mut A::Context, tx: Option<actix::dev::OneshotSender<Self>>) {
    if let Some(tx) = tx {
      if let Err(e) = tx.send(self) {
        error!("InvocationResponse can't be sent for tx_id {}", e.tx_id());
      }
    }
  }
}

impl Invocation {
  pub fn uuid() -> String {
    format!("{}", Uuid::new_v4())
  }
  /// Creates an invocation with a specific transaction id, to correlate a chain of
  /// invocations.
  pub fn next(
    tx_id: &str,
    hostkey: &KeyPair,
    origin: VinoEntity,
    target: VinoEntity,
    msg: impl Into<MessageTransport>,
  ) -> Invocation {
    let invocation_id = Invocation::uuid();
    let issuer = hostkey.public_key();
    let target_url = target.url();
    let payload = msg.into();
    let claims = Claims::<wascap::prelude::Invocation>::new(
      issuer.to_string(),
      invocation_id.to_string(),
      &target_url,
      &origin.url(),
      &invocation_hash(&target_url, &origin.url(), &payload),
    );
    Invocation {
      origin,
      target,
      msg: payload,
      id: invocation_id,
      encoded_claims: claims.encode(hostkey).unwrap(),
      host_id: issuer,
      tx_id: tx_id.to_string(),
    }
  }
}

pub(crate) fn invocation_hash(
  target_url: &str,
  origin_url: &str,
  msg: &MessageTransport,
) -> String {
  use std::io::Write;
  let mut cleanbytes: Vec<u8> = Vec::new();
  cleanbytes.write_all(origin_url.as_bytes()).unwrap();
  cleanbytes.write_all(target_url.as_bytes()).unwrap();
  match msg {
    MessageTransport::MessagePack(bytes) => cleanbytes.write_all(bytes).unwrap(),
    MessageTransport::Exception(string) => cleanbytes.write_all(string.as_bytes()).unwrap(),
    MessageTransport::Error(string) => cleanbytes.write_all(string.as_bytes()).unwrap(),
    MessageTransport::MultiBytes(bytemap) => {
      for (key, val) in bytemap {
        cleanbytes.write_all(key.as_bytes()).unwrap();
        cleanbytes.write_all(val).unwrap();
      }
    }
    MessageTransport::Test(v) => cleanbytes.write_all(v.as_bytes()).unwrap(),
    MessageTransport::Invalid => cleanbytes.write_all(&[0, 0, 0, 0, 0]).unwrap(),
    MessageTransport::OutputMap(map) => {
      for (key, val) in map {
        cleanbytes.write_all(key.as_bytes()).unwrap();
        cleanbytes
          .write_all(invocation_hash(origin_url, target_url, val).as_bytes())
          .unwrap();
      }
    }
  }
  let digest = sha256_digest(cleanbytes.as_slice()).unwrap();
  HEXUPPER.encode(digest.as_ref())
}

fn sha256_digest<R: Read>(mut reader: R) -> Result<Digest> {
  let mut context = Context::new(&SHA256);
  let mut buffer = [0; 1024];

  loop {
    let count = reader.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    context.update(&buffer[..count]);
  }

  Ok(context.finish())
}

#[derive(Debug, Clone, Serialize, Deserialize, Message, PartialEq)]
#[rtype(result = "InvocationResponse")]
/// The entity being referenced in an invocation.
pub enum VinoEntity {
  Test(String),
  Schematic(String),
  Component(ComponentEntity),
  Provider(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComponentEntity {
  pub id: String,
  pub reference: String,
  pub name: String,
}

impl Display for ComponentEntity {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}/{}", self.reference, self.id)
  }
}

impl Default for VinoEntity {
  fn default() -> Self {
    Self::Test("default".to_string())
  }
}

impl Display for VinoEntity {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.url())
  }
}

pub(crate) const URL_SCHEME: &str = "wasmbus";

impl VinoEntity {
  /// The URL of the entity
  pub fn url(&self) -> String {
    match self {
      VinoEntity::Test(name) => format!("{}://test/{}", URL_SCHEME, name),
      VinoEntity::Schematic(name) => format!("{}://schematic/{}", URL_SCHEME, name),
      VinoEntity::Component(e) => format!("{}://component/{}", URL_SCHEME, e.id),
      VinoEntity::Provider(name) => format!("{}://provider/{}", URL_SCHEME, name),
    }
  }

  /// The unique (public) key of the entity
  pub fn key(&self) -> String {
    match self {
      VinoEntity::Test(name) => format!("test:{}", name),
      VinoEntity::Schematic(name) => format!("schematic:{}", name),
      VinoEntity::Component(e) => format!("component:{}", e.id),
      VinoEntity::Provider(name) => format!("provider:{}", name),
    }
  }

  pub fn into_provider(self) -> Result<String> {
    match self {
      VinoEntity::Provider(s) => Ok(s),
      _ => Err(Error::ConversionError("into_provider")),
    }
  }

  pub fn into_component(self) -> Result<ComponentEntity> {
    match self {
      VinoEntity::Component(s) => Ok(s),
      _ => Err(Error::ConversionError("into_component")),
    }
  }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, Eq, Hash)]
pub struct PortEntity {
  pub schematic: String,
  pub reference: String,
  pub name: String,
}

impl PortEntity {
  pub fn new(schematic: String, reference: String, name: String) -> Self {
    Self {
      schematic,
      reference,
      name,
    }
  }
}

impl Display for PortEntity {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}/{}[{}]", self.schematic, self.reference, self.name)
  }
}
