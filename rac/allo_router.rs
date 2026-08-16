// cona/io/rac/allo_router.rs
//
// RAC Router with Alliag Cognitive Routing
//
// Era 5 Architecture:
//   RAC (Router-Alliag-Capability) protocol integrates cognitive routing
//   with FIM's Alliag variants for intelligent request dispatch.
//
// RAC Protocols:
//   RACAS - Human → FIM (Alliha): Direct intent from human operator
//   RACAG - Remote Agent → FIM (RemoteAlliag): Cloud AI inference
//   RACI  - Local Alliag → FEM: Intent compilation and execution
//   RACIN - Same-process FEM: FFI/shared memory
//   RACEX - Cloud AI → FIM: External inference services
//
// Cognitive Routing uses CI/CE metrics to classify entities:
//   CI = 0.30K + 0.40I + 0.20C + 0.10(100-H)
//   CE = 0.40E + 0.25A + 0.15D + 0.20S
//
// CI×CE Matrix Classification:
//   static_artifact (CI=0, CE=0)  - No inference, no execution
//   cona_fem (CI=0, CE>0)         - Execution only (pure FEM)
//   allio (CI>0, CE=0)            - Inference only (pure Alliag)
//   balanced_ape (CI>0, CE>0)     - Full APE (FIM + FEM)

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{string::{String, ToString}, vec::Vec, boxed::Box, sync::Arc};
use alloc::sync::Arc;

// Re-export from FIM for CI/CE types
#[cfg(feature = "fim")]
use hace_cona_fim::{
    CiMetrics, CeMetrics, SioType, SioEnvelope, SioPriority,
    Intent, Reasoning, Execute, Legal, Finance, Memory, Data, Vision,
    Alliag, AlliagKind, AlliagPlan, AlliagConfig,
    LocalAlliag, RemoteAlliag, AllihaHuman, StaticAlliag,
    IntentCompiler,
};

// ─── RAC Protocol Types ──────────────────────────────────────────────────────

/// RAC Protocol variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RacProtocol {
    /// RACAS: Human → FIM (Alliha) - Direct intent from human
    Racas,
    /// RACAG: Remote Agent → FIM (RemoteAlliag) - Cloud AI inference
    Racag,
    /// RACI: Local Alliag → FEM - Intent compilation and execution
    Raci,
    /// RACIN: Same-process FEM - FFI/shared memory
    Racin,
    /// RACEX: Cloud AI → FIM - External inference services
    Racex,
    /// RACID: In-device IPC - Named pipe/UDS
    Racid,
}

impl RacProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            RacProtocol::Racas => "RACAS",
            RacProtocol::Racag => "RACAG",
            RacProtocol::Raci => "RACI",
            RacProtocol::Racin => "RACIN",
            RacProtocol::Racex => "RACEX",
        }
    }
    
    /// Returns true if this protocol involves remote communication
    pub fn is_remote(&self) -> bool {
        matches!(self, RacProtocol::Racag | RacProtocol::Racex)
    }
    
    /// Returns true if this protocol involves human input
    pub fn is_human_input(&self) -> bool {
        matches!(self, RacProtocol::Racas)
    }
    
    /// Returns true if this protocol produces executable output
    pub fn produces_executable(&self) -> bool {
        matches!(self, RacProtocol::Raci | RacProtocol::Racin | RacProtocol::Racid)
    }
}

/// RAC Channel state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    Idle,
    Connecting,
    Connected,
    Authenticating,
    Active,
    Reconnecting,
    Error,
    Closed,
}

/// RAC Channel security configuration
#[derive(Debug, Clone)]
pub struct ChannelSecurity {
    pub encrypted: bool,
    pub auth_method: AuthMethod,
    pub certificate_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    None,
    Token,
    MutualTls,
    Jwt,
    OAuth2,
}

/// RAC routing decision with cognitive context
#[derive(Debug, Clone)]
pub struct RacRouteDecision {
    /// Target protocol/channel
    pub protocol: RacProtocol,
    /// Target endpoint (if remote)
    pub endpoint: Option<String>,
    /// CI metrics at time of routing
    pub ci_metrics: Option<CiMetrics>,
    /// CE metrics at time of routing
    pub ce_metrics: Option<CeMetrics>,
    /// Classification based on CI×CE matrix
    pub classification: EntityClassification,
    /// Priority of the route
    pub priority: RoutePriority,
    /// Retry policy
    pub retry_policy: RetryPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityClassification {
    /// CI=0, CE=0 - No inference, no execution (static artifact)
    StaticArtifact,
    /// CI=0, CE>0 - Execution only (pure FEM)
    ConaFem,
    /// CI>0, CE=0 - Inference only (pure Alliag)
    Allio,
    /// CI>0, CE>0 - Full APE (FIM + FEM)
    BalancedApe,
    /// CI>0, CE>0, high autonomy
    AutonomousApe,
    /// CI>0, CE>0, high execution capacity
    ExecutionHeavy,
    /// CI>0, CE>0, high inference capacity
    InferenceHeavy,
}

impl EntityClassification {
    /// Classify based on CI and CE scores
    pub fn from_ci_ce(ci: f32, ce: f32) -> Self {
        if ci < 10.0 && ce < 10.0 {
            return EntityClassification::StaticArtifact;
        }
        if ci < 10.0 {
            return EntityClassification::ConaFem;
        }
        if ce < 10.0 {
            return EntityClassification::Allio;
        }
        
        // Both CI and CE are significant
        if ci > 70.0 && ce > 70.0 {
            EntityClassification::BalancedApe
        } else if ci > 70.0 {
            EntityClassification::InferenceHeavy
        } else if ce > 70.0 {
            EntityClassification::ExecutionHeavy
        } else {
            EntityClassification::BalancedApe
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutePriority {
    Critical,
    High,
    Normal,
    Low,
    Background,
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u8,
    pub base_delay_ms: u32,
    pub max_delay_ms: u32,
    pub exponential_backoff: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 25,
            max_delay_ms: 5000,
            exponential_backoff: true,
        }
    }
}

// ─── Alliag Cognitive Router ─────────────────────────────────────────────────

/// AlliagCognitiveRouter - Routes requests based on Alliag variants and CI/CE metrics
pub struct AlliagCognitiveRouter {
    /// Local Alliag (GGUF) for on-device inference
    local_alliag: Option<Box<dyn Alliag>>,
    /// Remote Alliag (Cloud AI) for external inference
    remote_alliag: Option<Box<dyn Alliag>>,
    /// Human Alliha for direct human input
    alliha: Option<Box<dyn Alliag>>,
    /// Intent compiler (Local Alliag only)
    intent_compiler: IntentCompiler,
    /// CI metrics
    ci_metrics: Option<CiMetrics>,
    /// CE metrics
    ce_metrics: Option<CeMetrics>,
}

impl AlliagCognitiveRouter {
    pub fn new() -> Self {
        Self {
            local_alliag: None,
            remote_alliag: None,
            alliha: None,
            intent_compiler: IntentCompiler::with_defaults(),
            ci_metrics: None,
            ce_metrics: None,
        }
    }
    
    /// Initialize with local Alliag (GGUF)
    pub fn with_local_alliag(mut self, alliag: Box<dyn Alliag>) -> Self {
        self.local_alliag = Some(alliag);
        self
    }
    
    /// Initialize with remote Alliag (Cloud AI)
    pub fn with_remote_alliag(mut self, alliag: Box<dyn Alliag>) -> Self {
        self.remote_alliag = Some(alliag);
        self
    }
    
    /// Initialize with human Alliha
    pub fn with_alliha(mut self, alliag: Box<dyn Alliag>) -> Self {
        self.alliha = Some(alliag);
        self
    }
    
    /// Update CI metrics
    pub fn update_ci(&mut self, metrics: CiMetrics) {
        self.ci_metrics = Some(metrics);
    }
    
    /// Update CE metrics
    pub fn update_ce(&mut self, metrics: CeMetrics) {
        self.ce_metrics = Some(metrics);
    }
    
    /// Route an incoming request to the appropriate Alliag variant
    /// Returns the appropriate RAC protocol and routing decision
    pub fn route_intent(&self, intent: &Intent) -> RacRouteDecision {
        // Determine which Alliag to use based on intent characteristics
        let (protocol, classification, endpoint) = self.select_alliag_for_intent(intent);
        
        RacRouteDecision {
            protocol,
            endpoint,
            ci_metrics: self.ci_metrics.clone(),
            ce_metrics: self.ce_metrics.clone(),
            classification,
            priority: self.intent_to_priority(intent),
            retry_policy: RetryPolicy::default(),
        }
    }
    
    /// Route a reasoning result to execution
    pub fn route_reasoning(&self, reasoning: &Reasoning) -> RacRouteDecision {
        // Reasoning always goes through Local Alliag for compilation
        let classification = self.classify_current_entity();
        
        RacRouteDecision {
            protocol: RacProtocol::Raci,
            endpoint: None,
            ci_metrics: self.ci_metrics.clone(),
            ce_metrics: self.ce_metrics.clone(),
            classification,
            priority: RoutePriority::Normal,
            retry_policy: RetryPolicy::default(),
        }
    }
    
    /// Compile Intent + Reasoning to Execute (Local Alliag only)
    pub fn compile_to_execute(&self, intent: &Intent, reasoning: &Reasoning) -> Result<Execute, CompilerError> {
        self.intent_compiler.compile(intent, reasoning)
    }
    
    /// Select the appropriate Alliag variant for an intent
    fn select_alliag_for_intent(&self, intent: &Intent) -> (RacProtocol, EntityClassification, Option<String>) {
        use super::sio::IntentType;
        
        match intent.intent_type {
            // Human commands go through Alliha (RACAS)
            IntentType::Command | IntentType::Approval => {
                if self.alliha.is_some() {
                    (RacProtocol::Racas, self.classify_current_entity(), None)
                } else {
                    // Fallback to local Alliag
                    (RacProtocol::Raci, self.classify_current_entity(), None)
                }
            }
            // Queries can go to remote Alliag (RACAG) for better inference
            IntentType::Query => {
                if self.remote_alliag.is_some() {
                    (RacProtocol::Racag, self.classify_current_entity(), Some("cloud-ai://inference".to_string()))
                } else if self.local_alliag.is_some() {
                    (RacProtocol::Raci, self.classify_current_entity(), None)
                } else {
                    (RacProtocol::Racas, self.classify_current_entity(), None)
                }
            }
            // Planning goes to local Alliag for compilation
            IntentType::Planning => {
                (RacProtocol::Raci, self.classify_current_entity(), None)
            }
            // Negotiation can use remote for complex reasoning
            IntentType::Negotiation => {
                if self.remote_alliag.is_some() {
                    (RacProtocol::Racex, self.classify_current_entity(), Some("cloud-ai://negotiation".to_string()))
                } else {
                    (RacProtocol::Raci, self.classify_current_entity(), None)
                }
            }
        }
    }
    
    /// Classify the current entity based on CI/CE metrics
    fn classify_current_entity(&self) -> EntityClassification {
        let ci = self.ci_metrics.as_ref().map(|m| m.calculate()).unwrap_or(0.0);
        let ce = self.ce_metrics.as_ref().map(|m| m.calculate()).unwrap_or(0.0);
        EntityClassification::from_ci_ce(ci, ce)
    }
    
    /// Convert intent priority to route priority
    fn intent_to_priority(&self, intent: &Intent) -> RoutePriority {
        // Use intent constraints to determine priority
        for constraint in &intent.constraints {
            if constraint.name == "priority" {
                match constraint.value.as_str() {
                    "critical" => return RoutePriority::Critical,
                    "high" => return RoutePriority::High,
                    "low" => return RoutePriority::Low,
                    "background" => return RoutePriority::Background,
                    _ => {}
                }
            }
        }
        RoutePriority::Normal
    }
}

impl Default for AlliagCognitiveRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Compiler Error ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CompilerError {
    SafetyCheckFailed { reason: String },
    InvalidIntent { reason: String },
    InvalidReasoning { reason: String },
    CompilationFailed { reason: String },
}

impl CompilerError {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompilerError::SafetyCheckFailed { .. } => "safety_check_failed",
            CompilerError::InvalidIntent { .. } => "invalid_intent",
            CompilerError::InvalidReasoning { .. } => "invalid_reasoning",
            CompilerError::CompilationFailed { .. } => "compilation_failed",
        }
    }
}

// ─── SIO Extension for RAC ───────────────────────────────────────────────────

/// Extension trait for SioEnvelope to support RAC routing
pub trait SioRacExt {
    /// Get the appropriate RAC protocol for this SIO envelope
    fn rac_protocol(&self) -> RacProtocol;
    
    /// Check if this SIO requires compilation (Intent/Reasoning → Execute)
    fn requires_compilation(&self) -> bool;
    
    /// Check if this SIO is executable
    fn is_executable(&self) -> bool;
}

impl<T> SioRacExt for SioEnvelope<T> {
    fn rac_protocol(&self) -> RacProtocol {
        match self.sio_type {
            SioType::Intent => RacProtocol::Racas,
            SioType::Reasoning => RacProtocol::Raci,
            SioType::Execute => RacProtocol::Raci,
            SioType::Legal => RacProtocol::Raci,
            SioType::Finance => RacProtocol::Raci,
            SioType::Memory => RacProtocol::Racin,
            SioType::Data => RacProtocol::Racid,
            SioType::Vision => RacProtocol::Racex,
        }
    }
    
    fn requires_compilation(&self) -> bool {
        matches!(self.sio_type, SioType::Intent | SioType::Reasoning)
    }
    
    fn is_executable(&self) -> bool {
        matches!(self.sio_type, SioType::Execute)
    }
}

// ─── Mock Alliag for testing ─────────────────────────────────────────────────

/// Static Alliag implementation for testing
pub struct MockAlliag {
    kind: AlliagKind,
    config: AlliagConfig,
}

impl MockAlliag {
    pub fn new(kind: AlliagKind) -> Self {
        Self {
            kind,
            config: AlliagConfig::default(),
        }
    }
}

impl Alliag for MockAlliag {
    fn kind(&self) -> AlliagKind {
        self.kind
    }
    
    fn plan(&self, intent: &[u8]) -> Result<AlliagPlan, AlliagError> {
        Ok(AlliagPlan {
            steps: vec![],
            metadata: Default::default(),
        })
    }
    
    fn adapt(&self, _plan: &AlliagPlan, _step: usize, _output: &[u8]) -> Result<Vec<hace_cona_fim::PlanPatch>, AlliagError> {
        Ok(vec![])
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_entity_classification() {
        assert_eq!(
            EntityClassification::from_ci_ce(0.0, 0.0),
            EntityClassification::StaticArtifact
        );
        assert_eq!(
            EntityClassification::from_ci_ce(0.0, 50.0),
            EntityClassification::ConaFem
        );
        assert_eq!(
            EntityClassification::from_ci_ce(50.0, 0.0),
            EntityClassification::Allio
        );
        assert_eq!(
            EntityClassification::from_ci_ce(75.0, 75.0),
            EntityClassification::BalancedApe
        );
        assert_eq!(
            EntityClassification::from_ci_ce(80.0, 30.0),
            EntityClassification::InferenceHeavy
        );
        assert_eq!(
            EntityClassification::from_ci_ce(30.0, 80.0),
            EntityClassification::ExecutionHeavy
        );
    }
    
    #[test]
    fn test_rac_protocol() {
        assert!(RacProtocol::Racas.is_human_input());
        assert!(!RacProtocol::Raci.is_remote());
        assert!(RacProtocol::Racex.is_remote());
        assert!(RacProtocol::Raci.produces_executable());
    }
}