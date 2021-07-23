use std::env;
use std::path::PathBuf;

use tracing::debug;
use vino_manifest::error::ManifestError;
use vino_manifest::*;

#[test_env_log::test]
fn load_manifest_yaml() -> Result<(), ManifestError> {
  let path = PathBuf::from("./tests/manifests/v0/logger.yaml");
  let manifest = HostManifest::load_from_file(&path)?;

  let HostManifest::V0(manifest) = manifest;
  assert_eq!(manifest.default_schematic, "logger");

  Ok(())
}

#[test_env_log::test]
fn load_minimal() -> Result<(), ManifestError> {
  let path = PathBuf::from("./tests/manifests/v0/minimal.yaml");
  let manifest = HostManifest::load_from_file(&path)?;

  let HostManifest::V0(manifest) = manifest;
  assert_eq!(manifest.version, 0);

  Ok(())
}

#[test_env_log::test]
fn load_noversion_yaml() -> Result<(), ManifestError> {
  let path = PathBuf::from("./tests/manifests/v0/noversion.yaml");
  let result = HostManifest::load_from_file(&path);
  println!("result: {:?}", result);
  assert!(matches!(result, Err(ManifestError::NoVersion)));
  Ok(())
}

#[test_env_log::test]
fn load_bad_manifest_yaml() -> Result<(), ManifestError> {
  let path = PathBuf::from("./tests/manifests/v0/bad-yaml.yaml");
  let manifest = HostManifest::load_from_file(&path);
  if let Err(Error::YamlError(e)) = manifest {
    debug!("{:?}", e);
  } else {
    panic!("Should have failed with YamlError but got : {:?}", manifest);
  }

  Ok(())
}

#[test_env_log::test]
fn load_shortform_hocon() -> Result<(), ManifestError> {
  let path = PathBuf::from("./tests/manifests/v0/logger-shortform.manifest");
  let manifest = HostManifest::load_from_file(&path)?;

  let HostManifest::V0(manifest) = manifest;
  assert_eq!(manifest.default_schematic, "logger");
  let first_from = manifest.network.schematics[0].connections[0]
    .from
    .as_ref()
    .unwrap();
  let first_to = manifest.network.schematics[0].connections[0]
    .to
    .as_ref()
    .unwrap();
  assert_eq!(
    first_from,
    &v0::ConnectionTargetDefinition {
      instance: "<input>".to_owned(),
      port: "input".to_owned()
    }
  );
  assert_eq!(
    first_to,
    &v0::ConnectionTargetDefinition {
      instance: "logger".to_owned(),
      port: "input".to_owned()
    }
  );

  Ok(())
}

#[test_env_log::test]
fn load_shortform_yaml() -> Result<(), ManifestError> {
  let path = PathBuf::from("./tests/manifests/v0/logger-shortform.yaml");
  let manifest = HostManifest::load_from_file(&path)?;

  let HostManifest::V0(manifest) = manifest;
  assert_eq!(manifest.default_schematic, "logger");
  let first_from = manifest.network.schematics[0].connections[0]
    .from
    .as_ref()
    .unwrap();
  let first_to = manifest.network.schematics[0].connections[0]
    .to
    .as_ref()
    .unwrap();
  assert_eq!(
    first_from,
    &v0::ConnectionTargetDefinition {
      instance: "<input>".to_owned(),
      port: "input".to_owned()
    }
  );
  assert_eq!(
    first_to,
    &v0::ConnectionTargetDefinition {
      instance: "logger".to_owned(),
      port: "input".to_owned()
    }
  );

  Ok(())
}

#[test_env_log::test]
fn load_manifest_hocon() -> Result<(), ManifestError> {
  let path = PathBuf::from("./tests/manifests/v0/logger.manifest");
  let manifest = HostManifest::load_from_file(&path)?;

  let HostManifest::V0(manifest) = manifest;
  assert_eq!(manifest.default_schematic, "logger");

  Ok(())
}

#[test_env_log::test]

fn load_env() -> Result<(), ManifestError> {
  println!("Loading yaml");
  let path = PathBuf::from("./tests/manifests/v0/env.yaml");
  env::set_var("TEST_ENV_VAR", "load_manifest_yaml_with_env");
  let manifest = HostManifest::load_from_file(&path)?;

  let HostManifest::V0(manifest) = manifest;
  assert_eq!(
    manifest.network.schematics[0].name,
    "name_load_manifest_yaml_with_env"
  );
  println!("Loading hocon");
  let path = PathBuf::from("./tests/manifests/v0/env.manifest");
  env::set_var("TEST_ENV_VAR", "load_manifest_hocon_env");

  let manifest = HostManifest::load_from_file(&path)?;

  let HostManifest::V0(manifest) = manifest;
  assert_eq!(
    manifest.network.schematics[0].name,
    "name_load_manifest_hocon_env"
  );

  Ok(())
}

#[test_env_log::test]
fn load_bad_manifest_hocon() -> Result<(), ManifestError> {
  let path = PathBuf::from("./tests/manifests/v0/bad-hocon.manifest");
  let manifest = HostManifest::load_from_file(&path);
  if let Err(Error::HoconError(e)) = manifest {
    debug!("{:?}", e);
  } else {
    panic!("Should have failed")
  }

  Ok(())
}
