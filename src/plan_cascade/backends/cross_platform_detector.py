"""
Cross-Platform Agent Detection

Enhanced agent detection with platform-specific paths, common installation locations,
and caching support for efficient repeated lookups.
"""

import json
import os
import platform
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from enum import Enum
from pathlib import Path
from typing import Any


class Platform(Enum):
    """Supported operating system platforms."""
    WINDOWS = "windows"
    MACOS = "macos"
    LINUX = "linux"


@dataclass
class AgentInfo:
    """Information about a detected agent."""
    name: str
    available: bool
    path: str | None = None
    version: str | None = None
    platform: Platform | None = None
    detection_method: str | None = None
    last_checked: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "name": self.name,
            "available": self.available,
            "path": self.path,
            "version": self.version,
            "platform": self.platform.value if self.platform else None,
            "detection_method": self.detection_method,
            "last_checked": self.last_checked,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "AgentInfo":
        """Create from dictionary."""
        return cls(
            name=data["name"],
            available=data["available"],
            path=data.get("path"),
            version=data.get("version"),
            platform=Platform(data["platform"]) if data.get("platform") else None,
            detection_method=data.get("detection_method"),
            last_checked=data.get("last_checked"),
        )


@dataclass
class DetectorConfig:
    """Configuration for the cross-platform detector."""
    cache_ttl_hours: float = 1.0
    check_versions: bool = True
    use_registry: bool = True
    custom_paths: dict[str, list[str]] = field(default_factory=dict)


class CrossPlatformDetector:
    """
    Enhanced agent detection with platform-specific paths.

    Detects agents using multiple methods:
    1. PATH environment variable (shutil.which)
    2. Common installation locations per platform
    3. Windows registry (Windows only)
    4. Custom user-specified paths
    """

    COMMON_LOCATIONS: dict[Platform, dict[str, list[str]]] = {
        Platform.WINDOWS: {
            "codex": [
                "%LOCALAPPDATA%\\Programs\\codex\\codex.exe",
                "%APPDATA%\\npm\\codex.cmd",
                "%USERPROFILE%\\.local\\bin\\codex.exe",
            ],
            "aider": [
                "%USERPROFILE%\\.local\\bin\\aider.exe",
                "%APPDATA%\\Python\\Scripts\\aider.exe",
            ],
            "claude": [
                "%APPDATA%\\npm\\claude.cmd",
                "%USERPROFILE%\\.local\\bin\\claude.exe",
            ],
        },
        Platform.MACOS: {
            "codex": ["/usr/local/bin/codex", "/opt/homebrew/bin/codex", "~/.local/bin/codex"],
            "aider": ["/usr/local/bin/aider", "/opt/homebrew/bin/aider", "~/.local/bin/aider"],
            "claude": ["/usr/local/bin/claude", "/opt/homebrew/bin/claude"],
        },
        Platform.LINUX: {
            "codex": ["/usr/local/bin/codex", "/usr/bin/codex", "~/.local/bin/codex"],
            "aider": ["/usr/local/bin/aider", "/usr/bin/aider", "~/.local/bin/aider"],
            "claude": ["/usr/local/bin/claude", "~/.local/bin/claude"],
        },
    }

    VERSION_COMMANDS: dict[str, list[str]] = {
        "codex": ["--version"],
        "aider": ["--version"],
        "claude": ["--version"],
    }

    def __init__(
        self,
        config: DetectorConfig | None = None,
        cache_path: Path | None = None,
        project_root: Path | None = None,
    ):
        """Initialize the cross-platform detector."""
        self.config = config or DetectorConfig()
        self.project_root = Path(project_root) if project_root else Path.cwd()
        self.cache_path = cache_path or (self.project_root / ".agent-detection.json")
        self._cache: dict[str, AgentInfo] = {}
        self._platform: Platform | None = None
        self._load_cache()

    def detect_platform(self) -> Platform:
        """Detect the current operating system platform."""
        if self._platform is not None:
            return self._platform

        system = platform.system().lower()
        if system == "windows":
            self._platform = Platform.WINDOWS
        elif system == "darwin":
            self._platform = Platform.MACOS
        else:
            self._platform = Platform.LINUX

        return self._platform

    def detect_agent(self, agent_name: str, force_refresh: bool = False) -> AgentInfo:
        """Detect a single agent."""
        if not force_refresh and agent_name in self._cache:
            cached = self._cache[agent_name]
            if self._is_cache_valid(cached):
                return cached

        current_platform = self.detect_platform()
        now = datetime.now().isoformat()

        path, method = self._detect_agent_path(agent_name, current_platform)

        if path:
            version = self._get_version(agent_name, path) if self.config.check_versions else None
            info = AgentInfo(
                name=agent_name,
                available=True,
                path=path,
                version=version,
                platform=current_platform,
                detection_method=method,
                last_checked=now,
            )
        else:
            info = AgentInfo(
                name=agent_name,
                available=False,
                platform=current_platform,
                last_checked=now,
            )

        self._cache[agent_name] = info
        self._save_cache()
        return info

    def detect_all_agents(
        self,
        agent_names: list[str] | None = None,
        force_refresh: bool = False,
    ) -> dict[str, AgentInfo]:
        """Detect multiple agents."""
        if agent_names is None:
            agent_names = list(self.COMMON_LOCATIONS.get(self.detect_platform(), {}).keys())

        return {name: self.detect_agent(name, force_refresh) for name in agent_names}

    def get_available_agents(self, force_refresh: bool = False) -> list[AgentInfo]:
        """Get list of all available agents."""
        all_agents = self.detect_all_agents(force_refresh=force_refresh)
        return [info for info in all_agents.values() if info.available]

    def _detect_agent_path(
        self,
        agent_name: str,
        current_platform: Platform,
    ) -> tuple[str | None, str | None]:
        """Detect agent path using multiple methods."""
        # Method 1: Check PATH
        path = shutil.which(agent_name)
        if path:
            return os.path.abspath(path), "path"

        # Method 2: Check custom paths
        if agent_name in self.config.custom_paths:
            for custom_path in self.config.custom_paths[agent_name]:
                expanded = self._expand_path(custom_path)
                if expanded and os.path.isfile(expanded) and os.access(expanded, os.X_OK):
                    return expanded, "custom"

        # Method 3: Check common locations
        platform_locations = self.COMMON_LOCATIONS.get(current_platform, {})
        locations = platform_locations.get(agent_name, [])

        for location in locations:
            expanded = self._expand_path(location)
            if expanded and os.path.isfile(expanded) and os.access(expanded, os.X_OK):
                return expanded, "common_location"

        return None, None

    def _get_version(self, agent_name: str, path: str) -> str | None:
        """Get the version of an agent."""
        version_args = self.VERSION_COMMANDS.get(agent_name, ["--version"])

        try:
            kwargs: dict[str, Any] = {"capture_output": True, "text": True, "timeout": 10}
            if sys.platform == "win32":
                kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW

            result = subprocess.run([path] + version_args, **kwargs)
            if result.returncode == 0:
                output = result.stdout.strip() or result.stderr.strip()
                lines = output.split("\n")
                for line in lines:
                    if line.strip():
                        return line.strip()
        except (subprocess.TimeoutExpired, subprocess.SubprocessError, OSError):
            pass

        return None

    def _expand_path(self, path: str) -> str | None:
        """Expand environment variables and user home in path."""
        try:
            expanded = os.path.expandvars(path)
            return os.path.expanduser(expanded)
        except Exception:
            return None

    def _is_cache_valid(self, info: AgentInfo) -> bool:
        """Check if cached info is still valid based on TTL."""
        if not info.last_checked:
            return False

        try:
            last_checked = datetime.fromisoformat(info.last_checked)
            ttl = timedelta(hours=self.config.cache_ttl_hours)
            return datetime.now() - last_checked < ttl
        except (ValueError, TypeError):
            return False

    def _load_cache(self) -> None:
        """Load cache from disk."""
        if not self.cache_path.exists():
            return

        try:
            with open(self.cache_path, encoding="utf-8") as f:
                data = json.load(f)

            for agent_name, info_data in data.get("agents", {}).items():
                self._cache[agent_name] = AgentInfo.from_dict(info_data)
        except (json.JSONDecodeError, KeyError, TypeError):
            self._cache = {}

    def _save_cache(self) -> None:
        """Save cache to disk."""
        data = {
            "version": "1.0.0",
            "updated_at": datetime.now().isoformat(),
            "platform": self.detect_platform().value,
            "agents": {name: info.to_dict() for name, info in self._cache.items()},
        }

        try:
            with open(self.cache_path, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=2)
        except OSError:
            pass

    def clear_cache(self) -> None:
        """Clear the detection cache."""
        self._cache = {}
        if self.cache_path.exists():
            try:
                self.cache_path.unlink()
            except OSError:
                pass

    def get_detection_summary(self) -> dict[str, Any]:
        """Get a summary of all detected agents."""
        all_agents = self.detect_all_agents()
        available = [info for info in all_agents.values() if info.available]
        unavailable = [info for info in all_agents.values() if not info.available]

        return {
            "platform": self.detect_platform().value,
            "available_count": len(available),
            "unavailable_count": len(unavailable),
            "available": [
                {"name": info.name, "path": info.path, "version": info.version, "method": info.detection_method}
                for info in available
            ],
            "unavailable": [info.name for info in unavailable],
            "cache_path": str(self.cache_path),
        }
