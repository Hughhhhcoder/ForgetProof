use crate::model::{Capabilities, PROTOCOL_VERSION};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

pub struct AdapterClient {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<std::io::Result<String>>,
    next_id: u64,
}

impl AdapterClient {
    pub fn spawn(name: &str, mode: &str, config: &BTreeMap<String, String>) -> Result<Self> {
        let python = std::env::var("MEMORYPROOF_PYTHON")
            .or_else(|_| std::env::var("FORGETPROOF_PYTHON"))
            .unwrap_or_else(|_| {
                if cfg!(windows) {
                    "python".to_owned()
                } else {
                    "python3".to_owned()
                }
            });
        let module = match name {
            "reference-clean"
            | "reference-leaky"
            | "reference-overdelete"
            | "reference-slow"
            | "reference-crash"
            | "reference-malformed" => "forgetproof_adapters.reference",
            "mem0" => "forgetproof_adapters.mem0",
            "letta" => "forgetproof_adapters.letta",
            "zep" => "forgetproof_adapters.zep",
            other => bail!("unknown registered adapter '{other}'; use 'memoryproof adapters list'"),
        };

        let effective_mode = match name {
            "reference-clean" => "clean",
            "reference-leaky" => "leaky",
            "reference-overdelete" => "overdelete",
            "reference-slow" => "slow",
            "reference-crash" => "crash",
            "reference-malformed" => "malformed",
            _ => mode,
        };

        let config_json = serde_json::to_string(config)?;
        let mut command = Command::new(python);
        command
            .arg("-m")
            .arg(module)
            .env("MEMORYPROOF_ADAPTER_MODE", effective_mode)
            .env("FORGETPROOF_ADAPTER_MODE", effective_mode)
            .env("MEMORYPROOF_ADAPTER_NAME", name)
            .env("FORGETPROOF_ADAPTER_NAME", name)
            .env("MEMORYPROOF_CONFIG_JSON", &config_json)
            .env("FORGETPROOF_CONFIG_JSON", &config_json)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        if let Ok(current_dir) = std::env::current_dir() {
            let package_root = current_dir.join("python");
            if package_root.join("forgetproof_adapters").is_dir() {
                let mut paths = vec![package_root];
                if let Some(existing) = std::env::var_os("PYTHONPATH") {
                    paths.extend(std::env::split_paths(&existing));
                }
                let joined = std::env::join_paths(paths)
                    .context("failed to construct adapter PYTHONPATH")?;
                command.env("PYTHONPATH", joined);
            }
        }

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to start Python adapter '{name}'"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("adapter stdin was not piped"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("adapter stdout was not piped"))?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            lines: rx,
            next_id: 1,
        })
    }

    pub fn call(&mut self, method: &str, params: Value, timeout_ms: u64) -> Result<Value> {
        let id = self.next_id.to_string();
        self.next_id += 1;
        let request = json!({
            "protocol": PROTOCOL_VERSION,
            "id": id,
            "method": method,
            "params": params,
        });
        serde_json::to_writer(&mut self.stdin, &request)
            .with_context(|| format!("failed to write adapter request '{method}'"))?;
        self.stdin
            .write_all(b"\n")
            .with_context(|| format!("failed to terminate adapter request '{method}'"))?;
        self.stdin
            .flush()
            .with_context(|| format!("failed to flush adapter request '{method}'"))?;

        let line = self
            .lines
            .recv_timeout(Duration::from_millis(timeout_ms.max(1)))
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => {
                    anyhow!("adapter call '{method}' timed out after {timeout_ms}ms")
                }
                RecvTimeoutError::Disconnected => {
                    anyhow!("adapter exited while handling '{method}'")
                }
            })??;
        if line.trim().is_empty() {
            bail!("adapter emitted an empty response for '{method}'");
        }
        let response: Value = serde_json::from_str(&line)
            .with_context(|| format!("adapter emitted invalid JSON for '{method}'"))?;
        if response.get("id").and_then(Value::as_str) != Some(id.as_str()) {
            bail!("adapter response id mismatch for '{method}'");
        }
        if response.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(response.get("result").cloned().unwrap_or(Value::Null))
        } else {
            let error = response.get("error").cloned().unwrap_or(Value::Null);
            let code = error
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("adapter_error");
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("adapter returned an error");
            bail!("{code}: {message}");
        }
    }

    pub fn hello(&mut self, timeout_ms: u64) -> Result<Value> {
        let value = self.call("hello", json!({ "protocol": PROTOCOL_VERSION }), timeout_ms)?;
        let protocol = value
            .get("protocol")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("adapter hello response omitted protocol"))?;
        if protocol != PROTOCOL_VERSION {
            bail!(
                "protocol mismatch: runner expects {PROTOCOL_VERSION}, adapter returned {protocol}"
            );
        }
        Ok(value)
    }

    pub fn capabilities(&mut self, timeout_ms: u64) -> Result<Capabilities> {
        let value = self.call("capabilities", json!({}), timeout_ms)?;
        let capabilities: Capabilities =
            serde_json::from_value(value).context("invalid capabilities response")?;
        if capabilities.protocol != PROTOCOL_VERSION {
            bail!(
                "protocol mismatch in capabilities: expected {PROTOCOL_VERSION}, got {}",
                capabilities.protocol
            );
        }
        Ok(capabilities)
    }

    pub fn close(&mut self) {
        let _ = self.call("close", json!({}), 1_000);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for AdapterClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
