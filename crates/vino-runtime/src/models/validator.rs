use crate::dev::prelude::*;

type Result<T> = std::result::Result<T, ValidationError>;
pub(crate) struct Validator<'a> {
  model: &'a SchematicModel,
  omit_namespaces: Vec<String>,
}

fn should_omit(ns: &str, list: &[String]) -> bool {
  let omit = list.iter().find(|name| name == &ns);
  if omit.is_some() {
    trace!("Omitting {}", ns);
  }
  omit.is_some()
}

impl<'a> Validator<'a> {
  pub(crate) fn new(model: &'a SchematicModel, omit_namespaces: Vec<String>) -> Self {
    Validator {
      model,
      omit_namespaces,
    }
  }
  pub(crate) fn validate_early_errors(
    model: &'a SchematicModel,
  ) -> std::result::Result<(), ValidationError> {
    let validator = Validator::new(model, vec!["self".to_owned()]);
    let name = model.get_name();
    let results: Vec<ValidationError> = vec![
      validator.assert_early_schematic_inputs(),
      validator.assert_early_schematic_outputs(),
      validator.assert_early_qualified_names(),
      validator.assert_no_dangling_references(),
    ]
    .into_iter()
    .filter_map(std::result::Result::err)
    .collect();
    if results.is_empty() {
      Ok(())
    } else {
      Err(ValidationError::EarlyError(name, results))
    }
  }
  pub(crate) fn validate_late_errors(
    model: &'a SchematicModel,
  ) -> std::result::Result<(), ValidationError> {
    let validator = Validator::new(model, vec!["self".to_owned()]);
    let name = model.get_name();
    let results: Vec<ValidationError> = vec![
      validator.assert_component_models(),
      validator.assert_ports_used(),
    ]
    .into_iter()
    .filter_map(std::result::Result::err)
    .collect();
    if results.is_empty() {
      Ok(())
    } else {
      Err(ValidationError::PostInitError(name, results))
    }
  }
  pub(crate) fn validate_final_errors(
    model: &'a SchematicModel,
  ) -> std::result::Result<(), ValidationError> {
    let validator = Validator::new(model, vec![]);
    let name = model.get_name();
    let results: Vec<ValidationError> = vec![
      validator.assert_component_models(),
      validator.assert_ports_used(),
    ]
    .into_iter()
    .filter_map(std::result::Result::err)
    .collect();
    if results.is_empty() {
      Ok(())
    } else {
      Err(ValidationError::PostInitError(name, results))
    }
  }
  pub(crate) fn _validate(model: &'a SchematicModel) -> std::result::Result<(), ValidationError> {
    Self::validate_early_errors(model)?;
    Self::validate_late_errors(model)
  }
  fn assert_no_dangling_references(&self) -> Result<()> {
    let dangling: Vec<String> = self
      .model
      .get_connections()
      .iter()
      .flat_map(|conn| {
        let from = self
          .model
          .get_component_definition(conn.from.get_reference());
        let to = self.model.get_component_definition(conn.to.get_reference());
        let mut none = vec![];

        if from.is_none() && !conn.has_default() && !conn.from.matches_reference(SCHEMATIC_INPUT) {
          none.push(Some(conn.from.get_reference_owned()));
        }

        if to.is_none() && !conn.to.matches_reference(SCHEMATIC_OUTPUT) {
          none.push(Some(conn.to.get_reference_owned()));
        }
        none
      })
      .flatten()
      .collect();
    if dangling.is_empty() {
      Ok(())
    } else {
      Err(ValidationError::DanglingReference(dangling))
    }
  }
  fn assert_ports_used(&self) -> Result<()> {
    let errors: Vec<ValidationError> = self
      .model
      .get_connections()
      .iter()
      .flat_map(|connection| {
        let mut validations = vec![];
        if !connection.from.matches_reference(SCHEMATIC_INPUT) {
          let r = connection.from.get_reference();
          match self.model.get_component_model_by_ref(r) {
            Ok(from) => validations.push(is_valid_output(connection, &from)),
            Err(e) => {
              if !connection.has_default() && !self.should_omit_ref(r) {
                validations.push(Err(e.into()));
              }
            }
          };
        }
        if !connection.to.matches_reference(SCHEMATIC_OUTPUT) {
          let r = connection.to.get_reference();
          match self.model.get_component_model_by_ref(r) {
            Ok(to) => validations.push(is_valid_input(connection, &to)),
            Err(e) => {
              if !self.should_omit_ref(r) {
                validations.push(Err(e.into()));
              }
            }
          };
        }
        validations
      })
      .filter_map(Result::err)
      .collect();

    if errors.is_empty() {
      Ok(())
    } else {
      Err(ValidationError::InvalidConnections(errors))
    }
  }
  fn should_omit_ref(&self, reference: &str) -> bool {
    let option = self.model.get_component_definition(reference);
    option.map_or(false, |def| {
      should_omit(&def.namespace, &self.omit_namespaces)
    })
  }
  fn should_omit_def(&self, def: &ComponentDefinition) -> bool {
    should_omit(&def.namespace, &self.omit_namespaces)
  }

  fn assert_component_models(&self) -> Result<()> {
    let missing_components: Vec<String> = self
      .model
      .get_references()
      .filter_map(|r| {
        let def = self.model.get_component_definition(r).unwrap();

        let model = self.model.get_component_model(&def.id);
        (model.is_none() && !self.should_omit_def(&def))
          .then(|| format!("{}=>{}", r.clone(), def.id))
      })
      .collect();

    if missing_components.is_empty() {
      Ok(())
    } else {
      Err(ValidationError::MissingComponentModels(missing_components))
    }
  }
  fn assert_early_schematic_outputs(&self) -> Result<()> {
    let ports = self.model.get_schematic_outputs();
    if ports.is_empty() {
      Err(ValidationError::NoOutputs)
    } else {
      Ok(())
    }
  }

  // Validate that the passed port has an upstream that either connects
  // to the schematic input or a port that has a default
  fn validate_port_has_upstream_input(&self, port: &ConnectionTargetDefinition) -> bool {
    let connection = some_or_return!(self.model.get_upstream_connection(port), false);
    let connected_to_schematic_input = connection.from.matches_reference(SCHEMATIC_INPUT);
    let has_default = connection.from.is_none() && connection.has_default();
    if connected_to_schematic_input || has_default {
      return true;
    }

    let upstream_ref = connection.from.get_reference();
    let upstream_connections = self
      .model
      .get_upstream_connections_by_reference(upstream_ref);
    for conn in upstream_connections {
      if self.validate_port_has_upstream_input(&conn.to) {
        return true;
      }
    }
    false
  }

  fn assert_early_schematic_inputs(&self) -> Result<()> {
    let ports = self.model.get_schematic_outputs();
    for port in &ports {
      if !self.validate_port_has_upstream_input(port) {
        return Err(ValidationError::NoInputs);
      }
    }
    Ok(())
  }

  fn assert_early_qualified_names(&self) -> Result<()> {
    let component_definitions = self.model.get_component_definitions();
    let mut errors = vec![];
    for def in component_definitions {
      if parse_id(&def.id).is_err() {
        errors.push(def.id.clone());
      }
    }
    if errors.is_empty() {
      Ok(())
    } else {
      Err(ValidationError::NotFullyQualified(errors))
    }
  }
}

fn is_valid_input(connection: &ConnectionDefinition, to: &ComponentModel) -> Result<()> {
  let to_port = &connection.to;
  let found_to_port = to
    .inputs
    .iter()
    .find(|port| to_port.matches_port(&port.name));

  if found_to_port.is_none() {
    Err(ValidationError::InvalidInputPort(
      to_port.clone(),
      connection.clone(),
      to.inputs.clone(),
    ))
  } else {
    Ok(())
  }
}
fn is_valid_output(connection: &ConnectionDefinition, from: &ComponentModel) -> Result<()> {
  let from_port = &connection.from;
  let found_from_port = from
    .outputs
    .iter()
    .find(|port| from_port.matches_port(&port.name));

  if found_from_port.is_none() {
    Err(ValidationError::InvalidOutputPort(
      from_port.clone(),
      connection.clone(),
      from.outputs.clone(),
    ))
  } else {
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use vino_provider::ComponentSignature;
  use vino_rpc::{
    PortSignature,
    ProviderSignature,
  };
  use ConnectionTargetDefinition as Target;

  use super::*;
  use crate::test::prelude::*;
  type Result<T> = super::Result<T>;

  #[test_env_log::test]
  fn test_validate_early_errors() -> TestResult<()> {
    let def = load_network_manifest("./manifests/native-component.yaml")?;
    let model = SchematicModel::try_from(def.schematics[0].clone())?;

    Validator::validate_early_errors(&model)?;
    Ok(())
  }

  #[test_env_log::test]
  fn test_invalid_ports() -> TestResult<()> {
    let def = load_network_manifest("./manifests/invalid-bad-ports.yaml")?;
    let mut model = SchematicModel::try_from(def.schematics[0].clone())?;
    let expected_inputs = vec![PortSignature {
      name: "input".to_owned(),
      type_string: "string".to_owned(),
    }];
    let expected_outputs = vec![PortSignature {
      name: "output".to_owned(),
      type_string: "bytes".to_owned(),
    }];
    let provider = ProviderModel {
      namespace: "vino-v0".to_owned(),
      components: hashmap! {
        "log".to_owned() => ComponentModel {
          namespace: "test-namespace".to_owned(),
          name: "log".to_owned(),
          inputs: expected_inputs.clone(),
          outputs: expected_outputs.clone(),
        }
      },
    };
    model.commit_providers(vec![provider]);
    let result = Validator::validate_late_errors(&model);
    let first = &model.get_connections()[0];
    let second = &model.get_connections()[1];
    let expected = ValidationError::PostInitError(
      model.get_name(),
      vec![ValidationError::InvalidConnections(vec![
        ValidationError::InvalidInputPort(first.to.clone(), first.clone(), expected_inputs),
        ValidationError::InvalidOutputPort(second.from.clone(), second.clone(), expected_outputs),
      ])],
    );
    equals!(result, Err(expected));
    Ok(())
  }

  #[test_env_log::test]
  fn test_self() -> TestResult<()> {
    // the "self" namespace can't be validated until the non-self parts of every schematic are complete;
    let def = load_network_manifest("./manifests/reference-self.yaml")?;
    let mut model = SchematicModel::try_from(def.schematics[0].clone())?;
    let expected_inputs = vec![PortSignature {
      name: "input".to_owned(),
      type_string: "string".to_owned(),
    }];
    let expected_outputs = vec![PortSignature {
      name: "output".to_owned(),
      type_string: "bytes".to_owned(),
    }];
    let provider = ProviderModel {
      namespace: "vino".to_owned(),
      components: hashmap! {
        "log".to_owned() => ComponentModel {
          namespace: "test-namespace".to_owned(),
          name: "log".to_owned(),
          inputs: expected_inputs,
          outputs: expected_outputs,
        }
      },
    };
    model.commit_providers(vec![provider]);
    let result = Validator::validate_early_errors(&model);
    equals!(result, Ok(()));
    let result = Validator::validate_late_errors(&model);
    equals!(result, Ok(()));
    let provider = ProviderModel {
      namespace: "self".to_owned(),
      components: hashmap! {
        "child".to_owned() => ComponentModel {
          namespace: "child_ref".to_owned(),
          name: "child".to_owned(),
          inputs: vec![PortSignature {
            name: "child_input".to_owned(),
            type_string: "string".to_owned(),
          }],
          outputs: vec![PortSignature {
            name: "child_output".to_owned(),
            type_string: "bytes".to_owned(),
          }],
        }
      },
    };
    model.commit_providers(vec![provider]);
    let result = Validator::validate_final_errors(&model);
    equals!(result, Ok(()));

    Ok(())
  }

  #[test_env_log::test]
  fn test_connections() -> Result<()> {
    let schematic_name = "Test";
    let mut schematic_def = new_schematic(schematic_name);
    schematic_def.providers.push(ProviderDefinition {
      namespace: "test-namespace".to_owned(),
      kind: ProviderKind::Native,
      reference: "internal".to_owned(),
      data: HashMap::new(),
    });
    schematic_def.components.insert(
      "logger".to_owned(),
      ComponentDefinition::new("test-namespace", "log"),
    );
    schematic_def.connections.push(ConnectionDefinition {
      from: Target::new(SCHEMATIC_INPUT, "input"),
      to: Target::new("logger", "input"),
      default: None,
    });
    schematic_def.connections.push(ConnectionDefinition {
      from: Target::new("logger", "output"),
      to: Target::new(SCHEMATIC_OUTPUT, "output"),
      default: None,
    });
    let model = SchematicModel::try_from(schematic_def)?;
    equals!(model.get_name(), schematic_name);

    let upstream = model
      .get_upstream(&Target::new("logger".to_owned(), "input".to_owned()))
      .unwrap();
    equals!(upstream.get_reference(), SCHEMATIC_INPUT);
    equals!(upstream.get_port(), "input");

    Ok(())
  }

  #[test_env_log::test]
  fn test_dangling_refs() -> Result<()> {
    let schematic_name = "Test";
    let mut schematic_def = new_schematic(schematic_name);
    schematic_def.connections.push(ConnectionDefinition {
      from: Target::new("dangling1", "output"),
      to: Target::new(SCHEMATIC_OUTPUT, "output"),
      default: None,
    });
    let model = SchematicModel::try_from(schematic_def)?;
    equals!(model.get_name(), schematic_name);
    let result = Validator::validate_early_errors(&model);
    equals!(
      result,
      Err(ValidationError::EarlyError(
        "Test".to_owned(),
        vec![
          ValidationError::NoInputs,
          ValidationError::DanglingReference(vec!["dangling1".to_owned()]),
        ]
      ))
    );

    Ok(())
  }

  #[test_env_log::test]
  fn test_no_upstream() -> Result<()> {
    let schematic_name = "Test";
    let mut schematic_def = new_schematic(schematic_name);
    schematic_def.connections.push(ConnectionDefinition {
      from: Target::none(),
      to: Target::new(SCHEMATIC_OUTPUT, "output"),
      default: Some(serde_json::Value::String("Default string".to_owned())),
    });
    let model = SchematicModel::try_from(schematic_def)?;
    equals!(model.get_name(), schematic_name);
    let result = Validator::validate_early_errors(&model);
    equals!(result, Ok(()));
    let result = Validator::validate_late_errors(&model);
    equals!(result, Ok(()));

    Ok(())
  }

  #[test_env_log::test]
  fn test_missing_models() -> Result<()> {
    let schematic_name = "Test";
    let mut schematic_def = new_schematic(schematic_name);
    schematic_def.components.insert(
      "logger".to_owned(),
      ComponentDefinition::new("test-namespace", "log"),
    );
    let model = SchematicModel::try_from(schematic_def)?;
    let result = Validator::validate_late_errors(&model);
    equals!(
      result,
      Err(ValidationError::PostInitError(
        "Test".to_owned(),
        vec![ValidationError::MissingComponentModels(vec![
          "logger=>test-namespace::log".to_owned()
        ]),]
      ))
    );
    println!("{:?}", model);

    Ok(())
  }

  #[test_env_log::test]
  fn test_finish_initialization() -> TestResult<()> {
    let schematic_name = "Test";
    let mut schematic_def = new_schematic(schematic_name);
    schematic_def.providers.push(ProviderDefinition {
      namespace: "test-namespace".to_owned(),
      kind: ProviderKind::Native,
      reference: "internal".to_owned(),
      data: HashMap::new(),
    });
    schematic_def.components.insert(
      "logger".to_owned(),
      ComponentDefinition::new("test-namespace", "log"),
    );
    schematic_def.connections.push(ConnectionDefinition::new(
      Target::new(SCHEMATIC_INPUT, "input"),
      Target::new("logger", "input"),
    ));
    schematic_def.connections.push(ConnectionDefinition::new(
      Target::new("logger", "output"),
      Target::new(SCHEMATIC_OUTPUT, "output"),
    ));
    let mut model = SchematicModel::try_from(schematic_def)?;
    let provider = ProviderModel {
      namespace: "test-namespace".to_owned(),
      components: hashmap! {
        "log".to_owned() => ComponentModel {
          namespace: "test-namespace".to_owned(),
          name: "log".to_owned(),
          inputs: vec![PortSignature{name: "input".to_owned(), type_string: "string".to_owned()}],
          outputs: vec![PortSignature{name: "output".to_owned(), type_string: "bytes".to_owned()}],
        }
      },
    };
    model.commit_providers(vec![provider]);
    let result = Validator::_validate(&model);
    equals!(result, Ok(()));
    model.partial_initialization()?;
    let schematic_inputs = model.get_schematic_input_signatures()?;
    equals!(
      schematic_inputs,
      &vec![PortSignature {
        name: "input".to_owned(),
        type_string: "string".to_owned()
      }]
    );
    let schematic_outputs = model.get_schematic_output_signatures()?;
    equals!(
      schematic_outputs,
      &vec![PortSignature {
        name: "output".to_owned(),
        type_string: "bytes".to_owned()
      }]
    );
    let provider_sigs = model.get_provider_signatures()?;
    equals!(provider_sigs.len(), 1);
    equals!(
      provider_sigs[0],
      ProviderSignature {
        name: "test-namespace".to_owned(),
        components: vec![ComponentSignature {
          name: "log".to_owned(),
          inputs: vec![PortSignature {
            name: "input".to_owned(),
            type_string: "string".to_owned()
          }],
          outputs: vec![PortSignature {
            name: "output".to_owned(),
            type_string: "bytes".to_owned()
          }]
        }]
      }
    );

    Ok(())
  }
}