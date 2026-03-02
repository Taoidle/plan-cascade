//! Memory Query Policy V2
//!
//! Centralized tuning presets for unified memory retrieval. All call sites
//! should pull `top_k_total`, `min_importance`, and `per_scope_budget` from
//! this module instead of hardcoding values.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryQueryPresetV2 {
    CommandQuery,
    CommandSearch,
    CommandList,
    CommandStats,
    TaskContextAuto,
    TaskContextSelectionOnly,
    ContextEnvelopeAuto,
    ContextEnvelopeSelectionOnly,
    HookSessionStart,
    HookExtractionScan,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryQueryTuningV2 {
    pub top_k_total: usize,
    pub min_importance: f32,
    pub per_scope_budget: usize,
}

pub const DEFAULT_TOP_K_TOTAL_V2: usize = 20;
pub const DEFAULT_MIN_IMPORTANCE_V2: f32 = 0.1;
pub const DEFAULT_PER_SCOPE_BUDGET_V2: usize = 16;

pub fn memory_query_tuning_v2(preset: MemoryQueryPresetV2) -> MemoryQueryTuningV2 {
    match preset {
        MemoryQueryPresetV2::CommandQuery => MemoryQueryTuningV2 {
            top_k_total: DEFAULT_TOP_K_TOTAL_V2,
            min_importance: DEFAULT_MIN_IMPORTANCE_V2,
            per_scope_budget: DEFAULT_PER_SCOPE_BUDGET_V2,
        },
        MemoryQueryPresetV2::CommandSearch => MemoryQueryTuningV2 {
            top_k_total: 10,
            min_importance: 0.1,
            per_scope_budget: 24,
        },
        MemoryQueryPresetV2::CommandList => MemoryQueryTuningV2 {
            top_k_total: 50,
            min_importance: 0.0,
            per_scope_budget: 200,
        },
        MemoryQueryPresetV2::CommandStats => MemoryQueryTuningV2 {
            top_k_total: 20,
            min_importance: 0.0,
            per_scope_budget: 200,
        },
        MemoryQueryPresetV2::TaskContextAuto => MemoryQueryTuningV2 {
            top_k_total: 30,
            min_importance: 0.3,
            per_scope_budget: 16,
        },
        MemoryQueryPresetV2::TaskContextSelectionOnly => MemoryQueryTuningV2 {
            top_k_total: 50,
            min_importance: 0.0,
            per_scope_budget: 50,
        },
        MemoryQueryPresetV2::ContextEnvelopeAuto => MemoryQueryTuningV2 {
            top_k_total: 20,
            min_importance: 0.3,
            per_scope_budget: 12,
        },
        MemoryQueryPresetV2::ContextEnvelopeSelectionOnly => MemoryQueryTuningV2 {
            top_k_total: 50,
            min_importance: 0.0,
            per_scope_budget: 50,
        },
        MemoryQueryPresetV2::HookSessionStart => MemoryQueryTuningV2 {
            top_k_total: 50,
            min_importance: 0.1,
            per_scope_budget: 24,
        },
        MemoryQueryPresetV2::HookExtractionScan => MemoryQueryTuningV2 {
            top_k_total: 200,
            min_importance: 0.0,
            per_scope_budget: 80,
        },
    }
}

pub fn tuning_for_task_context_v2(has_selected_ids: bool) -> MemoryQueryTuningV2 {
    if has_selected_ids {
        memory_query_tuning_v2(MemoryQueryPresetV2::TaskContextSelectionOnly)
    } else {
        memory_query_tuning_v2(MemoryQueryPresetV2::TaskContextAuto)
    }
}

pub fn tuning_for_context_envelope_v2(has_selected_ids: bool) -> MemoryQueryTuningV2 {
    if has_selected_ids {
        memory_query_tuning_v2(MemoryQueryPresetV2::ContextEnvelopeSelectionOnly)
    } else {
        memory_query_tuning_v2(MemoryQueryPresetV2::ContextEnvelopeAuto)
    }
}
