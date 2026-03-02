//! Context pipeline event constants.
//!
//! Keep event names centralized so emitters and dashboard aggregations stay
//! consistent across refactors.

pub const TRACE_EVENT_COLLECT_START: &str = "collect_start";
pub const TRACE_EVENT_ROLLOUT_ASSIGNMENT: &str = "rollout_assignment";
pub const TRACE_EVENT_POLICY_NOTICE: &str = "policy_notice";
pub const TRACE_EVENT_CHAOS_CONFIG: &str = "chaos_config";
pub const TRACE_EVENT_SOURCE_COLLECTED: &str = "source_collected";
pub const TRACE_EVENT_SOURCE_SKIPPED: &str = "source_skipped";
pub const TRACE_EVENT_SOURCE_FAILED: &str = "source_failed";
pub const TRACE_EVENT_COMPACTION: &str = "compaction";
pub const TRACE_EVENT_ASSEMBLE_DONE: &str = "assemble_done";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraceEventType {
    CollectStart,
    RolloutAssignment,
    PolicyNotice,
    ChaosConfig,
    SourceCollected,
    SourceSkipped,
    SourceFailed,
    Compaction,
    AssembleDone,
}

impl TraceEventType {
    pub fn as_str(self) -> &'static str {
        match self {
            TraceEventType::CollectStart => TRACE_EVENT_COLLECT_START,
            TraceEventType::RolloutAssignment => TRACE_EVENT_ROLLOUT_ASSIGNMENT,
            TraceEventType::PolicyNotice => TRACE_EVENT_POLICY_NOTICE,
            TraceEventType::ChaosConfig => TRACE_EVENT_CHAOS_CONFIG,
            TraceEventType::SourceCollected => TRACE_EVENT_SOURCE_COLLECTED,
            TraceEventType::SourceSkipped => TRACE_EVENT_SOURCE_SKIPPED,
            TraceEventType::SourceFailed => TRACE_EVENT_SOURCE_FAILED,
            TraceEventType::Compaction => TRACE_EVENT_COMPACTION,
            TraceEventType::AssembleDone => TRACE_EVENT_ASSEMBLE_DONE,
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            TRACE_EVENT_COLLECT_START => Some(TraceEventType::CollectStart),
            TRACE_EVENT_ROLLOUT_ASSIGNMENT => Some(TraceEventType::RolloutAssignment),
            TRACE_EVENT_POLICY_NOTICE => Some(TraceEventType::PolicyNotice),
            TRACE_EVENT_CHAOS_CONFIG => Some(TraceEventType::ChaosConfig),
            TRACE_EVENT_SOURCE_COLLECTED => Some(TraceEventType::SourceCollected),
            TRACE_EVENT_SOURCE_SKIPPED => Some(TraceEventType::SourceSkipped),
            TRACE_EVENT_SOURCE_FAILED => Some(TraceEventType::SourceFailed),
            TRACE_EVENT_COMPACTION => Some(TraceEventType::Compaction),
            TRACE_EVENT_ASSEMBLE_DONE => Some(TraceEventType::AssembleDone),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_event_type_round_trip() {
        let variants = [
            TraceEventType::CollectStart,
            TraceEventType::RolloutAssignment,
            TraceEventType::PolicyNotice,
            TraceEventType::ChaosConfig,
            TraceEventType::SourceCollected,
            TraceEventType::SourceSkipped,
            TraceEventType::SourceFailed,
            TraceEventType::Compaction,
            TraceEventType::AssembleDone,
        ];

        for variant in variants {
            let value = variant.as_str();
            assert_eq!(Some(variant), TraceEventType::from_str(value));
        }

        assert_eq!(None, TraceEventType::from_str("unknown"));
    }
}
