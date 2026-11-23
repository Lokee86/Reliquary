//! Optional signed-in Codex client for the synthetic model pilot.
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

pub(super) struct SignedInSol {
    pub(super) executable: PathBuf,
    pub(super) scratch: PathBuf,
    pub(super) model: String,
}

impl GeneralEndpoint for SignedInSol {
    fn model(&self) -> &str {
        &self.model
    }

    fn complete_json(
        &self,
        prompt: &str,
        payload: &str,
        _name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let err = |_| GeneralEndpointError::Failure("trial client I/O failed".into());
        let dir = self.scratch.join(format!(
            "{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).map_err(err)?;
        let schema_file = dir.join("schema.json");
        let response_file = dir.join("response.json");
        fs::write(&schema_file, schema.to_string()).map_err(err)?;
        let mut cmd = Command::new(&self.executable);
        cmd.current_dir(&dir)
            .args([
                "exec",
                "--ephemeral",
                "--ignore-user-config",
                "--skip-git-repo-check",
                "--sandbox",
                "read-only",
                "--color",
                "never",
                "-m",
                &self.model,
                "-c",
                "model_reasoning_effort=\"low\"",
                "--output-schema",
            ])
            .arg(&schema_file)
            .arg("-o")
            .arg(&response_file)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                fs::File::create(dir.join("client.log")).map_err(err)?,
            ));
        let mut child = cmd.spawn().map_err(err)?;
        let mut input = child.stdin.take().expect("requested piped input");
        let written = write!(
            input,
            "{prompt}\nUse only the supplied synthetic data. Return the requested JSON.\n\n{payload}"
        );
        drop(input);
        let status = child.wait().map_err(err)?;
        written.map_err(err)?;
        if !status.success() {
            return Err(GeneralEndpointError::Failure(
                "trial client request failed".into(),
            ));
        }
        serde_json::from_slice(&fs::read(response_file).map_err(err)?).map_err(|_| {
            GeneralEndpointError::InvalidResponse("trial client returned invalid JSON")
        })
    }
}
