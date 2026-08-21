use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const PROTOCOL_VERSION: &str = "memoryproof.adapter/v1";
#[allow(dead_code)]
pub const LEGACY_PROTOCOL_VERSION: &str = "forgetproof.adapter/v1alpha1";
pub const API_VERSION: &str = "memoryproof.dev/v1";
pub const LEGACY_API_VERSION: &str = "forgetproof.dev/v1alpha1";
pub const BUNDLE_FORMAT: &str = "memoryproof.bundle/v1";

fn default_suite() -> String {
    "erasure".to_owned()
}

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
    vec!["erasure.object".to_owned()]
}

fn default_fixture_kind() -> String {
    "memory".to_owned()
}

fn default_target_subject() -> String {
    "target".to_owned()
}

fn default_control_subject() -> String {
    "control".to_owned()
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
    #[serde(default = "default_suite")]
    pub suite: String,
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
    #[serde(default = "default_target_subject")]
    pub target_subject: String,
    #[serde(default = "default_control_subject")]
    pub control_subject: String,
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
            target_subject: default_target_subject(),
            control_subject: default_control_subject(),
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
    pub role: String,
    #[serde(default)]
    pub target: bool,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub namespace: String,
    #[serde(default = "default_fixture_kind")]
    pub kind: String,
}

impl Fixture {
    pub fn is_target(&self) -> bool {
        self.role == "target" || (self.role.is_empty() && self.target)
    }

    pub fn is_control(&self) -> bool {
        self.role == "control" || (self.role.is_empty() && !self.target)
    }

    pub fn effective_subject(&self, isolation: &IsolationSpec) -> String {
        if !self.subject.is_empty() {
            return self.subject.clone();
        }
        if self.is_target() {
            isolation.target_subject.clone()
        } else {
            isolation.control_subject.clone()
        }
    }
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
    #[serde(default)]
    pub scope: String,
}

impl Default for EraseSpec {
    fn default() -> Self {
        Self {
            intent: default_intent(),
            target: default_target(),
            scope: String::new(),
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
    #[serde(default)]
    pub as_subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrivacySpec {
    #[serde(default)]
    pub raw_payloads: bool,
}

impl Scenario {
    pub fn is_legacy(&self) -> bool {
        self.api_version == LEGACY_API_VERSION
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.api_version != API_VERSION && self.api_version != LEGACY_API_VERSION {
            errors.push(format!(
                "apiVersion must be {API_VERSION} or {LEGACY_API_VERSION}, got {}",
                self.api_version
            ));
        }
        if self.metadata.name.trim().is_empty() {
            errors.push("metadata.name must not be empty".to_owned());
        }
        if self.spec.adapter.name.trim().is_empty() {
            errors.push("spec.adapter.name must not be empty".to_owned());
        }
        if self.spec.suite != "erasure" && self.spec.suite != "isolation" {
            errors.push(format!(
                "spec.suite must be 'erasure' or 'isolation', got '{}'",
                self.spec.suite
            ));
        }
        if self.spec.fixtures.is_empty() {
            errors.push("spec.fixtures must contain at least one fixture".to_owned());
        }
        let mut ids = std::collections::BTreeSet::new();
        for fixture in &self.spec.fixtures {
            if fixture.id.trim().is_empty() {
                errors.push("fixture id must not be empty".to_owned());
            }
            if !ids.insert(fixture.id.clone()) {
                errors.push(format!("duplicate fixture id '{}'", fixture.id));
            }
            if fixture.content.trim().is_empty() {
                errors.push(format!(
                    "fixture '{}' content must not be empty",
                    fixture.id
                ));
            }
            if !self.is_legacy() && fixture.role != "target" && fixture.role != "control" {
                errors.push(format!(
                    "fixture '{}' must declare role target or control",
                    fixture.id
                ));
            }
        }
        if !self.spec.fixtures.iter().any(Fixture::is_target) {
            errors.push("spec.fixtures must contain a target fixture".to_owned());
        }
        if self.spec.suite == "erasure"
            && !self
                .spec
                .fixtures
                .iter()
                .any(|fixture| fixture.id == self.spec.erase.target && fixture.is_target())
        {
            errors.push(format!(
                "erase.target '{}' must name a target fixture",
                self.spec.erase.target
            ));
        }
        if (self.spec.suite == "erasure" || self.spec.profiles.iter().any(|p| p.contains("scope")))
            && !self.spec.fixtures.iter().any(Fixture::is_control)
        {
            errors.push("scope and isolation checks require a control fixture".to_owned());
        }
        if self.spec.probes.before.is_empty() {
            errors.push("probes.before must contain probes".to_owned());
        }
        if self.spec.suite == "erasure" && self.spec.probes.after.is_empty() {
            errors.push("erasure scenarios require probes.after".to_owned());
        }
        for probe in self
            .spec
            .probes
            .before
            .iter()
            .chain(self.spec.probes.after.iter())
        {
            if probe.id.trim().is_empty() {
                errors.push("probe id must not be empty".to_owned());
            }
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
            if !["exact", "lexical", "semantic", "inspect", "agent"].contains(&probe.kind.as_str())
            {
                errors.push(format!(
                    "probe '{}' has unsupported kind '{}'",
                    probe.id, probe.kind
                ));
            }
        }
        if self.spec.profiles.is_empty() {
            errors.push("spec.profiles must contain at least one profile".to_owned());
        }
        for profile in &self.spec.profiles {
            if !supported_profile(profile, &self.spec.suite) {
                errors.push(format!(
                    "unsupported profile '{}' for suite '{}'",
                    profile, self.spec.suite
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn normalize_legacy(&mut self) {
        if !self.is_legacy() {
            return;
        }
        self.api_version = API_VERSION.to_owned();
        self.kind = if self.kind.is_empty() {
            "AssuranceScenario".to_owned()
        } else {
            self.kind.clone()
        };
        if self.spec.suite.is_empty() {
            self.spec.suite = "erasure".to_owned();
        }
        for fixture in &mut self.spec.fixtures {
            if fixture.role.is_empty() {
                fixture.role = if fixture.target { "target" } else { "control" }.to_owned();
            }
        }
        for profile in &mut self.spec.profiles {
            *profile = legacy_profile(profile);
        }
    }

    pub fn probe_subject(&self, probe: &Probe) -> String {
        if !probe.as_subject.is_empty() {
            return probe.as_subject.clone();
        }
        self.spec
            .fixtures
            .iter()
            .find(|fixture| fixture.id == probe.fixture)
            .map(|fixture| fixture.effective_subject(&self.spec.isolation))
            .unwrap_or_else(|| self.spec.isolation.target_subject.clone())
    }
}

pub fn legacy_profile(profile: &str) -> String {
    match profile {
        "FP-Object" => "erasure.object".to_owned(),
        "FP-Scope" => "erasure.scope".to_owned(),
        "FP-Derived" => "erasure.derived".to_owned(),
        "FP-Agent" => "erasure.agent".to_owned(),
        other => other.to_owned(),
    }
}

pub fn display_profile(profile: &str) -> &str {
    match profile {
        "erasure.object" => "MemoryProof · Erasure · Object",
        "erasure.scope" => "MemoryProof · Erasure · Scope",
        "erasure.derived" => "MemoryProof · Erasure · Derived",
        "erasure.agent" => "MemoryProof · Erasure · Agent",
        "isolation.read" => "IsolationProof · Read",
        "isolation.search" => "IsolationProof · Search",
        "isolation.agent" => "IsolationProof · Agent",
        other => other,
    }
}

pub fn supported_profile(profile: &str, suite: &str) -> bool {
    match suite {
        "erasure" => matches!(
            profile,
            "erasure.object"
                | "erasure.scope"
                | "erasure.derived"
                | "erasure.agent"
                | "FP-Object"
                | "FP-Scope"
                | "FP-Derived"
                | "FP-Agent"
        ),
        "isolation" => {
            matches!(
                profile,
                "isolation.read" | "isolation.search" | "isolation.agent"
            )
        }
        _ => false,
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
    #[serde(default)]
    pub modes: Vec<String>,
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
    pub artifact: String,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileResult {
    pub profile: String,
    #[serde(default)]
    pub label: String,
    pub status: String,
    pub assertions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub run_id: String,
    #[serde(default = "default_suite")]
    pub suite: String,
    pub scenario: String,
    pub adapter: String,
    pub backend: String,
    #[serde(default)]
    pub backend_version: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub scenario_hash: String,
    pub status: String,
    pub exit_code: i32,
    pub profiles: Vec<ProfileResult>,
    pub assertions: Vec<Assertion>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub out_of_scope: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

fn default_protocol() -> String {
    LEGACY_PROTOCOL_VERSION.to_owned()
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

pub fn profile_capabilities(profile: &str) -> &'static [&'static str] {
    match profile {
        "erasure.object" | "FP-Object" => &["object_delete", "probe"],
        "erasure.scope" | "FP-Scope" => &["scope_delete", "probe"],
        "erasure.derived" | "FP-Derived" => &["derived_inspect", "inspect"],
        "erasure.agent" | "FP-Agent" => &["agent_query"],
        "isolation.read" => &["isolated_namespace", "probe"],
        "isolation.search" => &["isolated_namespace", "lexical_search", "semantic_search"],
        "isolation.agent" => &["isolated_namespace", "agent_query"],
        _ => &[],
    }
}

pub fn profile_for_probe(probe: &Probe, suite: &str) -> String {
    if suite == "isolation" {
        return match probe.kind.as_str() {
            "agent" => "isolation.agent".to_owned(),
            "lexical" | "semantic" => "isolation.search".to_owned(),
            _ => "isolation.read".to_owned(),
        };
    }
    match probe.kind.as_str() {
        "inspect" => "erasure.derived".to_owned(),
        "agent" => "erasure.agent".to_owned(),
        _ => "erasure.object".to_owned(),
    }
}

pub fn erase_capability(intent: &str) -> &'static str {
    match intent {
        "subject_erase" => "scope_delete",
        "derived_purge" => "derived_delete",
        "access_revoke" => "access_revoke",
        _ => "object_delete",
    }
}

pub fn capability_available(capabilities: &Capabilities, capability: &str) -> bool {
    capabilities
        .capabilities
        .iter()
        .any(|item| item == capability)
}

pub fn normalize_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted = map
                .iter()
                .map(|(key, value)| (key.clone(), normalize_value(value)))
                .collect::<BTreeMap<_, _>>();
            serde_json::to_value(sorted).unwrap_or(Value::Null)
        }
        Value::Array(values) => Value::Array(values.iter().map(normalize_value).collect()),
        other => other.clone(),
    }
}
