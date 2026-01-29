"""
Settings Routes

Provides endpoints for managing Plan Cascade settings including
LLM backends, execution agents, and quality gates.
"""

import json
from dataclasses import asdict
from pathlib import Path
from typing import Any, Dict, List, Optional

from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel, Field

router = APIRouter()

# Settings file path
SETTINGS_FILE = Path.home() / ".plan-cascade" / "settings.json"


class AgentConfigModel(BaseModel):
    """Model for agent configuration."""
    name: str = Field(..., description="Agent identifier")
    enabled: bool = Field(default=True, description="Whether agent is enabled")
    command: str = Field(default="", description="Command to invoke agent")
    is_default: bool = Field(default=False, description="Whether this is the default agent")


class QualityGateConfigModel(BaseModel):
    """Model for quality gate configuration."""
    typecheck: bool = Field(default=True, description="Enable type checking")
    test: bool = Field(default=True, description="Enable test execution")
    lint: bool = Field(default=True, description="Enable linting")
    custom: bool = Field(default=False, description="Enable custom script")
    custom_script: str = Field(default="", description="Custom quality check script")
    max_retries: int = Field(default=3, description="Maximum retry attempts")


class LLMBackendConfigModel(BaseModel):
    """Model for LLM backend configuration."""
    backend: str = Field(default="claude-code", description="Backend type")
    provider: str = Field(default="claude", description="LLM provider name")
    model: str = Field(default="", description="Specific model to use")
    api_key_configured: bool = Field(default=False, description="Whether API key is set")


class SettingsModel(BaseModel):
    """Complete settings model."""
    # Backend configuration
    backend: str = Field(default="claude-code", description="Backend type")
    provider: str = Field(default="claude", description="LLM provider")
    model: str = Field(default="", description="Model name")

    # Agents
    agents: List[AgentConfigModel] = Field(default_factory=list)
    agent_selection: str = Field(default="prefer_default", description="Agent selection strategy")
    default_agent: str = Field(default="claude-code", description="Default agent name")

    # Quality gates
    quality_gates: QualityGateConfigModel = Field(default_factory=QualityGateConfigModel)

    # Execution configuration
    max_parallel_stories: int = Field(default=3, description="Max parallel stories")
    max_iterations: int = Field(default=50, description="Max iterations per story")
    timeout_seconds: int = Field(default=300, description="Story timeout")

    # UI configuration
    default_mode: str = Field(default="simple", description="Default UI mode")
    theme: str = Field(default="system", description="UI theme")


class SettingsUpdateModel(BaseModel):
    """Model for updating settings (all fields optional)."""
    backend: Optional[str] = None
    provider: Optional[str] = None
    model: Optional[str] = None
    agents: Optional[List[AgentConfigModel]] = None
    agent_selection: Optional[str] = None
    default_agent: Optional[str] = None
    quality_gates: Optional[QualityGateConfigModel] = None
    max_parallel_stories: Optional[int] = None
    max_iterations: Optional[int] = None
    timeout_seconds: Optional[int] = None
    default_mode: Optional[str] = None
    theme: Optional[str] = None


class APIKeyUpdateModel(BaseModel):
    """Model for updating API key."""
    provider: str = Field(..., description="Provider name (claude, openai, deepseek, ollama)")
    api_key: str = Field(..., description="API key value")


def _load_settings() -> Dict[str, Any]:
    """Load settings from file."""
    if SETTINGS_FILE.exists():
        with open(SETTINGS_FILE, "r") as f:
            return json.load(f)
    return _get_default_settings()


def _save_settings(settings: Dict[str, Any]) -> None:
    """Save settings to file."""
    SETTINGS_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(SETTINGS_FILE, "w") as f:
        json.dump(settings, f, indent=2)


def _get_default_settings() -> Dict[str, Any]:
    """Get default settings."""
    return {
        "backend": "claude-code",
        "provider": "claude",
        "model": "",
        "agents": [
            {"name": "claude-code", "enabled": True, "command": "claude", "is_default": True},
            {"name": "aider", "enabled": False, "command": "aider", "is_default": False},
            {"name": "codex", "enabled": False, "command": "codex", "is_default": False},
        ],
        "agent_selection": "prefer_default",
        "default_agent": "claude-code",
        "quality_gates": {
            "typecheck": True,
            "test": True,
            "lint": True,
            "custom": False,
            "custom_script": "",
            "max_retries": 3,
        },
        "max_parallel_stories": 3,
        "max_iterations": 50,
        "timeout_seconds": 300,
        "default_mode": "simple",
        "theme": "system",
    }


@router.get("/settings", response_model=SettingsModel)
async def get_settings() -> SettingsModel:
    """
    Get all settings.

    Returns the complete settings configuration including:
    - Backend and LLM configuration
    - Agent configurations
    - Quality gate settings
    - Execution limits
    - UI preferences
    """
    settings = _load_settings()

    # Convert to model (handles defaults for missing fields)
    return SettingsModel(
        backend=settings.get("backend", "claude-code"),
        provider=settings.get("provider", "claude"),
        model=settings.get("model", ""),
        agents=[AgentConfigModel(**a) for a in settings.get("agents", [])],
        agent_selection=settings.get("agent_selection", "prefer_default"),
        default_agent=settings.get("default_agent", "claude-code"),
        quality_gates=QualityGateConfigModel(**settings.get("quality_gates", {})),
        max_parallel_stories=settings.get("max_parallel_stories", 3),
        max_iterations=settings.get("max_iterations", 50),
        timeout_seconds=settings.get("timeout_seconds", 300),
        default_mode=settings.get("default_mode", "simple"),
        theme=settings.get("theme", "system"),
    )


@router.put("/settings")
async def update_settings(body: SettingsUpdateModel) -> Dict[str, Any]:
    """
    Update settings.

    Only updates fields that are provided. Omitted fields retain their current values.
    """
    settings = _load_settings()

    # Update only provided fields
    update_data = body.model_dump(exclude_none=True)

    # Handle nested objects
    if "agents" in update_data:
        update_data["agents"] = [a.model_dump() if hasattr(a, "model_dump") else a for a in update_data["agents"]]
    if "quality_gates" in update_data:
        qg = update_data["quality_gates"]
        update_data["quality_gates"] = qg.model_dump() if hasattr(qg, "model_dump") else qg

    settings.update(update_data)
    _save_settings(settings)

    return {"status": "updated", "settings": settings}


@router.post("/settings/reset")
async def reset_settings() -> Dict[str, Any]:
    """
    Reset settings to defaults.

    Restores all settings to their default values.
    """
    settings = _get_default_settings()
    _save_settings(settings)
    return {"status": "reset", "settings": settings}


# Agent management endpoints

@router.get("/settings/agents")
async def get_agents() -> Dict[str, Any]:
    """
    Get agent configurations.

    Returns all configured execution agents.
    """
    settings = _load_settings()
    return {
        "agents": settings.get("agents", []),
        "default_agent": settings.get("default_agent", "claude-code"),
        "agent_selection": settings.get("agent_selection", "prefer_default"),
    }


@router.put("/settings/agents")
async def update_agents(agents: List[AgentConfigModel]) -> Dict[str, Any]:
    """
    Update agent configurations.

    Replaces the entire agent list with the provided configurations.
    """
    settings = _load_settings()
    settings["agents"] = [a.model_dump() for a in agents]

    # Validate default agent exists
    agent_names = [a.name for a in agents]
    if settings.get("default_agent") not in agent_names:
        # Set first enabled agent as default
        for agent in agents:
            if agent.enabled:
                settings["default_agent"] = agent.name
                break

    _save_settings(settings)
    return {"status": "updated", "agents": settings["agents"]}


@router.post("/settings/agents")
async def add_agent(agent: AgentConfigModel) -> Dict[str, Any]:
    """
    Add a new agent configuration.
    """
    settings = _load_settings()
    agents = settings.get("agents", [])

    # Check for duplicate
    for existing in agents:
        if existing["name"] == agent.name:
            raise HTTPException(
                status_code=409,
                detail=f"Agent '{agent.name}' already exists"
            )

    # Handle is_default
    if agent.is_default:
        for existing in agents:
            existing["is_default"] = False
        settings["default_agent"] = agent.name

    agents.append(agent.model_dump())
    settings["agents"] = agents
    _save_settings(settings)

    return {"status": "added", "agent": agent.model_dump()}


@router.delete("/settings/agents/{agent_name}")
async def delete_agent(agent_name: str) -> Dict[str, Any]:
    """
    Delete an agent configuration.
    """
    settings = _load_settings()
    agents = settings.get("agents", [])

    # Find and remove agent
    new_agents = [a for a in agents if a["name"] != agent_name]
    if len(new_agents) == len(agents):
        raise HTTPException(
            status_code=404,
            detail=f"Agent '{agent_name}' not found"
        )

    # If deleting default agent, set a new default
    if settings.get("default_agent") == agent_name:
        for agent in new_agents:
            if agent.get("enabled"):
                settings["default_agent"] = agent["name"]
                agent["is_default"] = True
                break

    settings["agents"] = new_agents
    _save_settings(settings)

    return {"status": "deleted", "agent_name": agent_name}


# LLM backend endpoints

@router.get("/settings/llm")
async def get_llm_settings() -> Dict[str, Any]:
    """
    Get LLM backend configuration.
    """
    settings = _load_settings()

    # Check if API keys are configured (without exposing them)
    api_keys = _load_api_keys()

    return {
        "backend": settings.get("backend", "claude-code"),
        "provider": settings.get("provider", "claude"),
        "model": settings.get("model", ""),
        "available_backends": [
            {
                "id": "claude-code",
                "name": "Claude Code (Claude Max)",
                "requires_api_key": False,
                "description": "Use Claude Code as LLM backend (no API key required)",
            },
            {
                "id": "claude-api",
                "name": "Claude API",
                "requires_api_key": True,
                "api_key_configured": "claude" in api_keys,
                "description": "Direct Anthropic Claude API",
            },
            {
                "id": "openai",
                "name": "OpenAI",
                "requires_api_key": True,
                "api_key_configured": "openai" in api_keys,
                "description": "OpenAI GPT models",
            },
            {
                "id": "deepseek",
                "name": "DeepSeek",
                "requires_api_key": True,
                "api_key_configured": "deepseek" in api_keys,
                "description": "DeepSeek models",
            },
            {
                "id": "ollama",
                "name": "Ollama (Local)",
                "requires_api_key": False,
                "description": "Local Ollama instance",
            },
        ],
    }


@router.put("/settings/llm")
async def update_llm_settings(
    backend: Optional[str] = None,
    provider: Optional[str] = None,
    model: Optional[str] = None,
) -> Dict[str, Any]:
    """
    Update LLM backend configuration.
    """
    settings = _load_settings()

    if backend is not None:
        settings["backend"] = backend
    if provider is not None:
        settings["provider"] = provider
    if model is not None:
        settings["model"] = model

    _save_settings(settings)

    return {
        "status": "updated",
        "backend": settings["backend"],
        "provider": settings["provider"],
        "model": settings["model"],
    }


# API key management (secure storage)

API_KEYS_FILE = Path.home() / ".plan-cascade" / ".api_keys"


def _load_api_keys() -> Dict[str, str]:
    """Load API keys from secure storage."""
    if API_KEYS_FILE.exists():
        try:
            with open(API_KEYS_FILE, "r") as f:
                return json.load(f)
        except Exception:
            return {}
    return {}


def _save_api_keys(keys: Dict[str, str]) -> None:
    """Save API keys to secure storage."""
    API_KEYS_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(API_KEYS_FILE, "w") as f:
        json.dump(keys, f, indent=2)
    # Restrict file permissions (Unix only)
    try:
        import os
        os.chmod(API_KEYS_FILE, 0o600)
    except Exception:
        pass


@router.post("/settings/api-key")
async def set_api_key(body: APIKeyUpdateModel) -> Dict[str, Any]:
    """
    Set an API key for a provider.

    API keys are stored securely and not returned in responses.
    Supported providers: claude, openai, deepseek
    """
    valid_providers = ["claude", "openai", "deepseek"]
    if body.provider not in valid_providers:
        raise HTTPException(
            status_code=400,
            detail=f"Invalid provider. Must be one of: {', '.join(valid_providers)}"
        )

    # Try to use keyring for secure storage
    try:
        import keyring
        keyring.set_password("plan-cascade", f"{body.provider}_api_key", body.api_key)
    except ImportError:
        # Fall back to file-based storage
        keys = _load_api_keys()
        keys[body.provider] = body.api_key
        _save_api_keys(keys)

    return {"status": "saved", "provider": body.provider}


@router.delete("/settings/api-key/{provider}")
async def delete_api_key(provider: str) -> Dict[str, Any]:
    """
    Delete an API key for a provider.
    """
    try:
        import keyring
        keyring.delete_password("plan-cascade", f"{provider}_api_key")
    except ImportError:
        keys = _load_api_keys()
        if provider in keys:
            del keys[provider]
            _save_api_keys(keys)
    except Exception:
        pass

    return {"status": "deleted", "provider": provider}


# Quality gate endpoints

@router.get("/settings/quality-gates")
async def get_quality_gates() -> Dict[str, Any]:
    """
    Get quality gate configuration.
    """
    settings = _load_settings()
    return {
        "quality_gates": settings.get("quality_gates", {}),
        "available_gates": [
            {
                "id": "typecheck",
                "name": "Type Check",
                "description": "Run type checking (mypy, pyright, tsc)",
            },
            {
                "id": "test",
                "name": "Tests",
                "description": "Run tests (pytest, jest, npm test)",
            },
            {
                "id": "lint",
                "name": "Lint",
                "description": "Run linting (ruff, eslint)",
            },
            {
                "id": "custom",
                "name": "Custom Script",
                "description": "Run a custom quality check script",
            },
        ],
    }


@router.put("/settings/quality-gates")
async def update_quality_gates(body: QualityGateConfigModel) -> Dict[str, Any]:
    """
    Update quality gate configuration.
    """
    settings = _load_settings()
    settings["quality_gates"] = body.model_dump()
    _save_settings(settings)

    return {"status": "updated", "quality_gates": settings["quality_gates"]}


# Import/Export

@router.get("/settings/export")
async def export_settings() -> Dict[str, Any]:
    """
    Export settings for backup or transfer.

    Returns all settings (excluding API keys).
    """
    settings = _load_settings()
    return {
        "version": "1.0",
        "settings": settings,
    }


@router.post("/settings/import")
async def import_settings(data: Dict[str, Any]) -> Dict[str, Any]:
    """
    Import settings from a backup.

    Validates and applies the imported settings.
    """
    if "settings" not in data:
        raise HTTPException(
            status_code=400,
            detail="Invalid import format. Expected 'settings' key."
        )

    imported = data["settings"]

    # Validate required fields
    required_fields = ["backend", "provider"]
    for field in required_fields:
        if field not in imported:
            raise HTTPException(
                status_code=400,
                detail=f"Missing required field: {field}"
            )

    # Merge with defaults for any missing fields
    defaults = _get_default_settings()
    merged = {**defaults, **imported}

    _save_settings(merged)

    return {"status": "imported", "settings": merged}
