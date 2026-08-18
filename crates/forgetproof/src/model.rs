use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PROTOCOL_VERSION: &str = "forgetproof.adapter/v1alpha1";
pub const API_VERSION: &str = "forgetproof.dev/v1alpha1";

fn default_scope() -> String {
    "run".to_owned()
}

fn default_mode() -> String {
    "default".to_owned()
}

fn default_timeout() -> u64 {
    10_000
}

fn default_interval() -> u64 {
    250
}

fn default_intent() -> String {
    "object_delete".to_owned()
}

fn default_target() -> String {
    "target".to_owned()
}

fn default_probe_kind() -> String {
    "exact".to_owned()
}

fn default_profiles() -> Vec<String> {
    vec!["FP-Object".to_owned()]
}

fn default_fixture_kind() -> String {
    "memory".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    #[serde(default)]
    pub kind: String,
    pub metadata: Metadata,
    pub spec: ScenarioSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioSpec {
    pub adapter: AdapterSpec,
    #[serde(default)]
    pub isolation: IsolationSpec,
    #[serde(default)]
    pub fixtures: Vec<Fixture>,
    #[serde(default)]
    pub settle: SettleSpec,
    #[serde(default)]
    pub erase: EraseSpec,
    #[serde(default)]
    pub probes: ProbeSpec,
    #[serde(default = "default_profiles")]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub privacy: PrivacySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdapterSpec {
    pub name: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub config: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolationSpec {
    #[serde(default = "default_scope")]
    pub scope: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub thread: String,
}

impl Default for IsolationSpec {
    fn default() -> Self {
        Self {
            scope: default_scope(),
            subject: String::new(),
            agent: String::new(),
            thread: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fixture {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub target: bool,
    #[serde(default = "default_fixture_kind")]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettleSpec {
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "default_interval")]
    pub interval_ms: u64,
}

impl Default for SettleSpec {
    fn default() -> Self {
        Self {
            timeout_ms: default_timeout(),
            interval_ms: default_interval(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EraseSpec {
    #[serde(default = "default_intent")]
    pub intent: String,
    #[serde(default = "default_target")]
    pub target: String,
}

impl Default for EraseSpec {
    fn default() -> Self {
        Self {
            intent: default_intent(),
            target: default_target(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProbeSpec {
    #[serde(default)]
    pub before: Vec<Probe>,
    #[serde(default)]
    pub after: Vec<Probe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Probe {
    pub id: String,
    pub fixture: String,
    #[serde(default = "default_probe_kind")]
    pub kind: String,
    #[serde(default)]
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrivacySpec {
    #[serde(default)]
    pub raw_payloads: bool,
}

impl Scenario {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.api_version != API_VERSION {
            errors.push(format!(
                "apiVersion must be {API_VERSION}, got {}",
                self.api_version
            ));
        }
        if self.metadata.name.trim().is_empty() {
            errors.push("metadata.name must not be empty".to_owned());
        }
        if self.spec.adapter.name.trim().is_empty() {
            errors.push("spec.adapter.name must not be empty".to_owned());
        }
        if self.spec.fixtures.is_empty() {
            errors.push("spec.fixtures must contain at least one fixture".to_owned());
        }
        if !self.spec.fixtures.iter().any(|fixture| fixture.target) {
            errors.push("spec.fixtures must contain a target fixture".to_owned());
        }
        if !self
            .spec
            .fixtures
            .iter()
            .any(|fixture| fixture.id == self.spec.erase.target)
        {
            errors.push(format!(
                "erase.target '{}' does not name a fixture",
                self.spec.erase.target
            ));
        }
        if self.spec.probes.before.is_empty() || self.spec.probes.after.is_empty() {
            errors.push("probes.before and probes.after must both contain probes".to_owned());
        }
        for probe in self
            .spec
            .probes
            .before
            .iter()
            .chain(self.spec.probes.after.iter())
        {
            if !self
                .spec
                .fixtures
                .iter()
                .any(|fixture| fixture.id == probe.fixture)
            {
                errors.push(format!(
                    "probe '{}' references unknown fixture '{}'",
                    probe.id, probe.fixture
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Capabilities {
    pub protocol: String,
    pub adapter: String,
    #[serde(default)]
    pub backend: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assertion {
    pub id: String,
    pub profile: String,
    pub status: String,
    pub message: String,
    #[serde(default)]
    pub expected: Option<bool>,
    #[serde(default)]
    pub observed: Option<bool>,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileResult {
    pub profile: String,
    pub status: String,
    pub assertions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub run_id: String,
    pub scenario: String,
    pub adapter: String,
    pub backend: String,
    pub scenario_hash: String,
    pub status: String,
    pub exit_code: i32,
    pub profiles: Vec<ProfileResult>,
    pub assertions: Vec<Assertion>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub run_id: String,
    pub created_at_ms: u128,
    pub scenario_hash: String,
    pub adapter: String,
    pub backend: String,
    pub protocol: String,
    pub files: Vec<String>,
    #[serde(default)]
    pub bundle_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub at_ms: u128,
    pub phase: String,
    pub method: String,
    pub status: String,
    #[serde(default)]
    pub details: BTreeMap<String, String>,
}
