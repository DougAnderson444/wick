use std::fs;

pub(crate) use vino_interfaces_collection::list_items::*;
use vino_provider::Context;

pub(crate) async fn job(
  input: Inputs,
  output: Outputs,
  context: Context<crate::State>,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let state = context.lock().unwrap();
  let mut path = state.directory.clone();
  drop(state);
  path.push(input.collection_id);
  if !path.exists() {
    output
      .document_ids
      .done_exception(format!("No directory found at {}", path.to_string_lossy()));
    return Ok(());
  }

  let contents = fs::read_dir(path)?;
  output.document_ids.done(
    contents
      .filter_map(Result::ok)
      .map(|dir| dir.file_name().to_string_lossy().into())
      .collect(),
  );
  Ok(())
}
