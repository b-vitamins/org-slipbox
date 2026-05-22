use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use slipbox_rpc::{JsonRpcRequest, JsonRpcResponse, read_framed_message, write_framed_message};

use crate::config::DaemonServeConfig;
use crate::error::DaemonClientError;
use crate::rpc::JsonRpcTransport;

pub(crate) struct StdioTransport {
    child: Option<Child>,
    reader: Option<BufReader<ChildStdout>>,
    writer: Option<BufWriter<ChildStdin>>,
}

impl StdioTransport {
    pub(crate) fn spawn(
        program: impl Into<PathBuf>,
        config: &DaemonServeConfig,
    ) -> Result<Self, DaemonClientError> {
        let program = program.into();
        let mut command = Command::new(&program);
        command
            .args(config.command_args())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let child = command
            .spawn()
            .map_err(|source| DaemonClientError::StartDaemon {
                program: program.clone(),
                source,
            })?;
        Self::from_child(child)
    }

    pub(crate) fn from_child(mut child: Child) -> Result<Self, DaemonClientError> {
        let stdout = child
            .stdout
            .take()
            .ok_or(DaemonClientError::MissingStdout)?;
        let stdin = child.stdin.take().ok_or(DaemonClientError::MissingStdin)?;
        Ok(Self {
            child: Some(child),
            reader: Some(BufReader::new(stdout)),
            writer: Some(BufWriter::new(stdin)),
        })
    }
}

impl JsonRpcTransport for StdioTransport {
    fn round_trip(
        &mut self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, DaemonClientError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or(DaemonClientError::ConnectionClosed)?;
        write_framed_message(writer, &request)
            .map_err(|source| DaemonClientError::WriteRequest { source })?;

        let reader = self
            .reader
            .as_mut()
            .ok_or(DaemonClientError::ConnectionClosed)?;
        match read_framed_message(reader)
            .map_err(|source| DaemonClientError::ReadResponse { source })?
        {
            Some(response) => Ok(response),
            None => {
                if let Some(child) = self.child.as_mut() {
                    if let Some(status) = child
                        .try_wait()
                        .map_err(|source| DaemonClientError::Shutdown { source })?
                    {
                        Err(DaemonClientError::DaemonExited { status })
                    } else {
                        Err(DaemonClientError::UnexpectedEof)
                    }
                } else {
                    Err(DaemonClientError::ConnectionClosed)
                }
            }
        }
    }

    fn shutdown(&mut self) -> Result<(), DaemonClientError> {
        self.reader.take();
        self.writer.take();
        if let Some(mut child) = self.child.take() {
            child
                .wait()
                .map_err(|source| DaemonClientError::Shutdown { source })?;
        }
        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        self.reader.take();
        self.writer.take();
        if let Some(mut child) = self.child.take() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }
}
