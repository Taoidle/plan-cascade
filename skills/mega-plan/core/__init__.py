"""
Mega Plan Core Modules

Provides functionality for multi-task orchestration at the project level.
"""

from .mega_generator import MegaPlanGenerator
from .mega_state import MegaStateManager
from .feature_orchestrator import FeatureOrchestrator
from .merge_coordinator import MergeCoordinator

__all__ = [
    'MegaPlanGenerator',
    'MegaStateManager',
    'FeatureOrchestrator',
    'MergeCoordinator'
]
