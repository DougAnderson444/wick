use wick_interface_types as wick;

use super::*;

impl TryFrom<v1::TypeDefinition> for wick::TypeDefinition {
  type Error = ManifestError;

  fn try_from(value: v1::TypeDefinition) -> std::result::Result<Self, Self::Error> {
    Ok(match value {
      v1::TypeDefinition::StructSignature(v) => wick::TypeDefinition::Struct(v.try_into()?),
      v1::TypeDefinition::EnumSignature(v) => wick::TypeDefinition::Enum(v.try_into()?),
    })
  }
}

impl TryFrom<wick::TypeDefinition> for v1::TypeDefinition {
  type Error = ManifestError;

  fn try_from(value: wick::TypeDefinition) -> std::result::Result<Self, Self::Error> {
    Ok(match value {
      wick::TypeDefinition::Struct(v) => v1::TypeDefinition::StructSignature(v.try_into()?),
      wick::TypeDefinition::Enum(v) => v1::TypeDefinition::EnumSignature(v.try_into()?),
    })
  }
}

impl TryFrom<v1::StructSignature> for wick::StructSignature {
  type Error = ManifestError;

  fn try_from(value: v1::StructSignature) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      name: value.name,
      description: value.description,
      fields: value.fields.try_map_into()?,
      imported: false,
    })
  }
}

impl TryFrom<wick::StructSignature> for v1::StructSignature {
  type Error = ManifestError;

  fn try_from(value: wick::StructSignature) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      name: value.name,
      description: value.description,
      fields: value.fields.try_map_into()?,
    })
  }
}

impl TryFrom<v1::EnumSignature> for wick::EnumSignature {
  type Error = ManifestError;

  fn try_from(value: v1::EnumSignature) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      name: value.name,
      description: value.description,
      variants: value.variants.try_map_into()?,
      imported: false,
    })
  }
}

impl TryFrom<wick::EnumSignature> for v1::EnumSignature {
  type Error = ManifestError;

  fn try_from(value: wick::EnumSignature) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      name: value.name,
      description: value.description,
      variants: value.variants.try_map_into()?,
    })
  }
}

impl TryFrom<v1::EnumVariant> for wick::EnumVariant {
  type Error = ManifestError;

  fn try_from(value: v1::EnumVariant) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      name: value.name,
      description: value.description,
      index: value.index,
      value: value.value,
    })
  }
}

impl TryFrom<wick::EnumVariant> for v1::EnumVariant {
  type Error = ManifestError;

  fn try_from(value: wick::EnumVariant) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      name: value.name,
      description: value.description,
      index: value.index,
      value: value.value,
    })
  }
}

impl TryFrom<v1::Field> for wick::Field {
  type Error = ManifestError;

  fn try_from(value: v1::Field) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      name: value.name,
      description: value.description,
      ty: value.ty.try_into()?,
      default: None,
      required: true,
    })
  }
}

impl TryFrom<wick::Field> for v1::Field {
  type Error = ManifestError;

  fn try_from(value: wick::Field) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      name: value.name,
      description: value.description,
      ty: value.ty.try_into()?,
    })
  }
}

impl FromStr for v1::TypeSignature {
  type Err = ManifestError;

  fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
    wick::parse(s).map_err(ManifestError::TypeParser)?.try_into()
  }
}

impl TryFrom<v1::TypeSignature> for wick::TypeSignature {
  type Error = ManifestError;

  fn try_from(value: v1::TypeSignature) -> std::result::Result<Self, Self::Error> {
    use wick::TypeSignature as TS;
    let v = match value {
      v1::TypeSignature::I8(_) => TS::I8,
      v1::TypeSignature::I16(_) => TS::I16,
      v1::TypeSignature::I32(_) => TS::I32,
      v1::TypeSignature::I64(_) => TS::I64,
      v1::TypeSignature::U8(_) => TS::U8,
      v1::TypeSignature::U16(_) => TS::U16,
      v1::TypeSignature::U32(_) => TS::U32,
      v1::TypeSignature::U64(_) => TS::U64,
      v1::TypeSignature::F32(_) => TS::F32,
      v1::TypeSignature::F64(_) => TS::F64,
      v1::TypeSignature::Bool(_) => TS::Bool,
      v1::TypeSignature::StringType(_) => TS::String,
      v1::TypeSignature::Optional(t) => TS::Optional {
        ty: Box::new((*t.ty).try_into()?),
      },
      v1::TypeSignature::Datetime(_) => TS::Datetime,
      v1::TypeSignature::Bytes(_) => TS::Bytes,
      v1::TypeSignature::Custom(v) => TS::Custom(v.name),
      v1::TypeSignature::List(t) => TS::List {
        ty: Box::new((*t.ty).try_into()?),
      },
      v1::TypeSignature::Map(t) => TS::Map {
        key: Box::new((*t.key).try_into()?),
        value: Box::new((*t.value).try_into()?),
      },
      v1::TypeSignature::Object(_) => TS::Object,
    };
    Ok(v)
  }
}

impl TryFrom<wick::TypeSignature> for v1::TypeSignature {
  type Error = ManifestError;

  fn try_from(value: wick::TypeSignature) -> std::result::Result<Self, Self::Error> {
    use v1::TypeSignature as TS;
    let v = match value {
      wick::TypeSignature::I8 => TS::I8(v1::I8()),
      wick::TypeSignature::I16 => TS::I16(v1::I16()),
      wick::TypeSignature::I32 => TS::I32(v1::I32()),
      wick::TypeSignature::I64 => TS::I64(v1::I64()),
      wick::TypeSignature::U8 => TS::U8(v1::U8()),
      wick::TypeSignature::U16 => TS::U16(v1::U16()),
      wick::TypeSignature::U32 => TS::U32(v1::U32()),
      wick::TypeSignature::U64 => TS::U64(v1::U64()),
      wick::TypeSignature::F32 => TS::F32(v1::F32()),
      wick::TypeSignature::F64 => TS::F64(v1::F64()),
      wick::TypeSignature::Bool => TS::Bool(v1::Bool()),
      wick::TypeSignature::String => TS::StringType(v1::StringType()),
      wick::TypeSignature::Optional { ty } => TS::Optional(v1::Optional {
        ty: Box::new((*ty).try_into()?),
      }),
      wick::TypeSignature::Datetime => TS::Datetime(v1::Datetime {}),
      wick::TypeSignature::Bytes => TS::Bytes(v1::Bytes {}),
      wick::TypeSignature::Custom(v) => TS::Custom(v1::Custom { name: v }),
      wick::TypeSignature::List { ty } => TS::List(v1::List {
        ty: Box::new((*ty).try_into()?),
      }),
      wick::TypeSignature::Map { key, value } => TS::Map(v1::Map {
        key: Box::new((*key).try_into()?),
        value: Box::new((*value).try_into()?),
      }),
      wick::TypeSignature::Link { .. } => unimplemented!(),
      wick::TypeSignature::Object => TS::Object(v1::Object {}),
      wick::TypeSignature::Ref { .. } => unimplemented!(),
      wick::TypeSignature::AnonymousStruct(_) => unimplemented!(),
    };
    Ok(v)
  }
}