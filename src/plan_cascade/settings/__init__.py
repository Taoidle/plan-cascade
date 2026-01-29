"""
Settings management module for Plan Cascade.

This module provides configuration management including:
- Settings data models (BackendType, AgentConfig, QualityGateConfig, Settings)
- YAML-based configuration storage
- Secure API key storage using keyring
- Configuration validation
- Interactive setup wizard
- Configuration migration and versioning
"""

from .models import (
    BackendType,
    AgentConfig,
    QualityGateConfig,
    Settings,
)
from .storage import SettingsStorage
from .migration import ConfigMigration
from .validation import ConfigValidator, ValidationResult
from .wizard import SetupWizard, run_setup_wizard

__all__ = [
    # Models
    "BackendType",
    "AgentConfig",
    "QualityGateConfig",
    "Settings",
    # Storage
    "SettingsStorage",
    # Migration
    "ConfigMigration",
    # Validation
    "ConfigValidator",
    "ValidationResult",
    # Wizard
    "SetupWizard",
    "run_setup_wizard",
]
