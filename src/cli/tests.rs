use super::output::{CliCommandError, ErrorPayload, OutputMode, write_output};
use slipbox_core::StatusInfo;

#[test]
fn writes_json_output_from_structured_results() {
    let mut output = Vec::new();
    let status = StatusInfo {
        version: "0.7.0".to_owned(),
        root: "/tmp/root".to_owned(),
        db: "/tmp/db.sqlite".to_owned(),
        files_indexed: 1,
        nodes_indexed: 2,
        links_indexed: 3,
    };

    write_output(&mut output, OutputMode::Json, &status, |_| String::new())
        .expect("json output should render");

    let parsed: StatusInfo =
        serde_json::from_slice(&output).expect("json output should deserialize");
    assert_eq!(parsed.files_indexed, 1);
}

#[test]
fn writes_structured_json_errors() {
    let error = CliCommandError::new(OutputMode::Json, anyhow::anyhow!("broken"));
    let mut stderr = Vec::new();

    error.write(&mut stderr).expect("json error should render");

    let parsed: ErrorPayload =
        serde_json::from_slice(&stderr).expect("json error should deserialize");
    assert_eq!(parsed.error.message, "broken");
}
