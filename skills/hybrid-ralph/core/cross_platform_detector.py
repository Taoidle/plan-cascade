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
from dataclasses import dataclass, field, asdict
from datetime import datetime, timedelta
from enum import Enum
from pathlib import Path
from typing import Dict, List, Optional, Any


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
    path: Optional[str] = None
    version: Optional[str] = None
    platform: Optional[Platform] = None
    detection_method: Optional[str] = None  # "path", "common_location", "registry"
    last_checked: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
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
    def from_dict(cls, data: Dict[str, Any]) -> "AgentInfo":
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
    cache_ttl_hours: float = 1.0  # Cache TTL in hours
    check_versions: bool = True  # Whether to check agent versions
    use_registry: bool = True  # Whether to check Windows registry
    custom_paths: Dict[str, List[str]] = field(default_factory=dict)  # Additional paths per agent


class CrossPlatformDetector:
    """
    Enhanced agent detection with platform-specific paths.

    Detects agents using multiple methods:
    1. PATH environment variable (shutil.which)
    2. Common installation locations per platform
    3. Windows registry (Windows only)
    4. Custom user-specified paths

    Results are cached to avoid repeated filesystem checks.
    """

    # Common installation locations by platform
    COMMON_LOCATIONS: Dict[Platform, Dict[str, List[str]]] = {
        Platform.WINDOWS: {
            "codex": [
                "%LOCALAPPDATA%\\Programs\\codex\\codex.exe",
                "%APPDATA%\\npm\\codex.cmd",
                "%USERPROFILE%\\.local\\bin\\codex.exe",
                "C:\\Program Files\\codex\\codex.exe",
                "C:\\Program Files (x86)\\codex\\codex.exe",
            ],
            "aider": [
                "%USERPROFILE%\\.local\\bin\\aider.exe",
                "%APPDATA%\\Python\\Scripts\\aider.exe",
                "%LOCALAPPDATA%\\Programs\\Python\\Python3*\\Scripts\\aider.exe",
                "C:\\Python3*\\Scripts\\aider.exe",
            ],
            "cursor": [
                "%LOCALAPPDATA%\\Programs\\Cursor\\Cursor.exe",
                "%LOCALAPPDATA%\\Cursor\\Cursor.exe",
                "C:\\Program Files\\Cursor\\Cursor.exe",
            ],
            "cursor-cli": [
                "%LOCALAPPDATA%\\Programs\\Cursor\\resources\\app\\bin\\cursor.cmd",
                "%LOCALAPPDATA%\\Programs\\Cursor\\resources\\app\\bin\\code.cmd",
            ],
            "amp": [
                "%APPDATA%\\npm\\amp.cmd",
                "%USERPROFILE%\\.local\\bin\\amp.exe",
            ],
            "claude": [
                "%APPDATA%\\npm\\claude.cmd",
                "%USERPROFILE%\\.local\\bin\\claude.exe",
                "%LOCALAPPDATA%\\Programs\\claude\\claude.exe",
            ],
        },
        Platform.MACOS: {
            "codex": [
                "/usr/local/bin/codex",
                "/opt/homebrew/bin/codex",
                "~/.local/bin/codex",
                "~/.npm-global/bin/codex",
            ],
            "aider": [
                "/usr/local/bin/aider",
                "/opt/homebrew/bin/aider",
                "~/.local/bin/aider",
                "~/Library/Python/*/bin/aider",
            ],
            "cursor": [
                "/Applications/Cursor.app/Contents/MacOS/Cursor",
            ],
            "cursor-cli": [
                "/Applications/Cursor.app/Contents/Resources/app/bin/cursor",
                "/Applications/Cursor.app/Contents/Resources/app/bin/code",
                "/usr/local/bin/cursor",
            ],
            "amp": [
                "/usr/local/bin/amp",
                "/opt/homebrew/bin/amp",
                "~/.npm-global/bin/amp",
            ],
            "claude": [
                "/usr/local/bin/claude",
                "/opt/homebrew/bin/claude",
                "~/.npm-global/bin/claude",
            ],
        },
        Platform.LINUX: {
            "codex": [
                "/usr/local/bin/codex",
                "/usr/bin/codex",
                "~/.local/bin/codex",
                "~/.npm-global/bin/codex",
                "/snap/bin/codex",
            ],
            "aider": [
                "/usr/local/bin/aider",
                "/usr/bin/aider",
                "~/.local/bin/aider",
            ],
            "cursor": [
                "/usr/share/cursor/cursor",
                "/opt/cursor/cursor",
                "~/.local/share/cursor/cursor",
            ],
            "cursor-cli": [
                "/usr/share/cursor/resources/app/bin/cursor",
                "/opt/cursor/bin/cursor",
                "/usr/local/bin/cursor",
            ],
            "amp": [
                "/usr/local/bin/amp",
                "~/.local/bin/amp",
                "~/.npm-global/bin/amp",
            ],
            "claude": [
                "/usr/local/bin/claude",
                "~/.local/bin/claude",
                "~/.npm-global/bin/claude",
            ],
        },
    }

    # Version command patterns
    VERSION_COMMANDS: Dict[str, List[str]] = {
        "codex": ["--version"],
        "aider": ["--version"],
        "cursor": ["--version"],
        "cursor-cli": ["--version"],
        "amp": ["--version"],
        "claude": ["--version"],
    }

    # Windows registry keys for installed applications
    WINDOWS_REGISTRY_KEYS: Dict[str, List[str]] = {
        "cursor": [
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Cursor",
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Cursor",
        ],
    }

    def __init__(
        self,
        config: Optional[DetectorConfig] = None,
        cache_path: Optional[Path] = None,
        project_root: Optional[Path] = None,
    ):
        """
        Initialize the cross-platform detector.

        Args:
            config: Detector configuration
            cache_path: Path for cache file (defaults to project_root/.agent-detection.json)
            project_root: Project root directory
        """
        self.config = config or DetectorConfig()
        self.project_root = Path(project_root) if project_root else Path.cwd()
        self.cache_path = cache_path or (self.project_root / ".agent-detection.json")
        self._cache: Dict[str, AgentInfo] = {}
        self._platform: Optional[Platform] = None

        # Load existing cache
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
        """
        Detect a single agent.

        Args:
            agent_name: Name of the agent to detect
            force_refresh: If True, bypass cache and re-detect

        Returns:
            AgentInfo with detection results
        """
        # Check cache first
        if not force_refresh and agent_name in self._cache:
            cached = self._cache[agent_name]
            if self._is_cache_valid(cached):
                return cached

        current_platform = self.detect_platform()
        now = datetime.now().isoformat()

        # Try detection methods in order
        path, method = self._detect_agent_path(agent_name, current_platform)

        if path:
            version = None
            if self.config.check_versions:
                version = self._get_version(agent_name, path)

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

        # Update cache
        self._cache[agent_name] = info
        self._save_cache()

        return info

    def detect_all_agents(
        self,
        agent_names: Optional[List[str]] = None,
        force_refresh: bool = False,
    ) -> Dict[str, AgentInfo]:
        """
        Detect multiple agents.

        Args:
            agent_names: List of agent names to detect (defaults to all known agents)
            force_refresh: If True, bypass cache and re-detect all

        Returns:
            Dictionary mapping agent names to AgentInfo
        """
        if agent_names is None:
            # Detect all known agents
            agent_names = list(self.COMMON_LOCATIONS.get(self.detect_platform(), {}).keys())

        results = {}
        for name in agent_names:
            results[name] = self.detect_agent(name, force_refresh=force_refresh)

        return results

    def get_available_agents(self, force_refresh: bool = False) -> List[AgentInfo]:
        """Get list of all available agents."""
        all_agents = self.detect_all_agents(force_refresh=force_refresh)
        return [info for info in all_agents.values() if info.available]

    def _detect_agent_path(
        self,
        agent_name: str,
        current_platform: Platform,
    ) -> tuple[Optional[str], Optional[str]]:
        """
        Detect agent path using multiple methods.

        Returns:
            Tuple of (path, detection_method) or (None, None) if not found
        """
        # Method 1: Check PATH
        path = self._check_path(agent_name)
        if path:
            return path, "path"

        # Method 2: Check custom paths from config
        if agent_name in self.config.custom_paths:
            for custom_path in self.config.custom_paths[agent_name]:
                expanded = self._expand_path(custom_path)
                if expanded and os.path.isfile(expanded) and os.access(expanded, os.X_OK):
                    return expanded, "custom"

        # Method 3: Check common locations
        path = self._check_common_locations(agent_name, current_platform)
        if path:
            return path, "common_location"

        # Method 4: Check Windows registry (Windows only)
        if current_platform == Platform.WINDOWS and self.config.use_registry:
            path = self._check_windows_registry(agent_name)
            if path:
                return path, "registry"

        return None, None

    def _check_path(self, agent_name: str) -> Optional[str]:
        """Check if agent is available in PATH."""
        path = shutil.which(agent_name)
        if path:
            return os.path.abspath(path)
        return None

    def _check_common_locations(
        self,
        agent_name: str,
        current_platform: Platform,
    ) -> Optional[str]:
        """Check common installation locations for the agent."""
        platform_locations = self.COMMON_LOCATIONS.get(current_platform, {})
        locations = platform_locations.get(agent_name, [])

        for location in locations:
            expanded = self._expand_path(location)
            if expanded:
                # Handle glob patterns (e.g., Python3*)
                if "*" in expanded:
                    matches = self._glob_paths(expanded)
                    for match in matches:
                        if os.path.isfile(match) and os.access(match, os.X_OK):
                            return match
                elif os.path.isfile(expanded) and os.access(expanded, os.X_OK):
                    return expanded

        return None

    def _check_windows_registry(self, agent_name: str) -> Optional[str]:
        """Check Windows registry for installed applications."""
        if not sys.platform == "win32":
            return None

        try:
            import winreg
        except ImportError:
            return None

        registry_keys = self.WINDOWS_REGISTRY_KEYS.get(agent_name, [])

        for key_path in registry_keys:
            for hive in [winreg.HKEY_LOCAL_MACHINE, winreg.HKEY_CURRENT_USER]:
                try:
                    with winreg.OpenKey(hive, key_path) as key:
                        install_location, _ = winreg.QueryValueEx(key, "InstallLocation")
                        if install_location:
                            # Try to find the executable
                            exe_names = [f"{agent_name}.exe", f"{agent_name}.cmd", agent_name]
                            for exe_name in exe_names:
                                exe_path = os.path.join(install_location, exe_name)
                                if os.path.isfile(exe_path):
                                    return exe_path
                except (FileNotFoundError, OSError):
                    continue

        return None

    def _get_version(self, agent_name: str, path: str) -> Optional[str]:
        """Get the version of an agent."""
        version_args = self.VERSION_COMMANDS.get(agent_name, ["--version"])

        try:
            # Set up subprocess options for platform
            kwargs: Dict[str, Any] = {
                "capture_output": True,
                "text": True,
                "timeout": 10,
            }

            if sys.platform == "win32":
                kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW

            result = subprocess.run([path] + version_args, **kwargs)

            if result.returncode == 0:
                output = result.stdout.strip() or result.stderr.strip()
                # Extract version number (simple pattern matching)
                lines = output.split("\n")
                if lines:
                    # Return first non-empty line
                    for line in lines:
                        line = line.strip()
                        if line:
                            return line
        except (subprocess.TimeoutExpired, subprocess.SubprocessError, OSError):
            pass

        return None

    def _expand_path(self, path: str) -> Optional[str]:
        """Expand environment variables and user home in path."""
        try:
            # Expand environment variables
            expanded = os.path.expandvars(path)
            # Expand user home
            expanded = os.path.expanduser(expanded)
            return expanded
        except Exception:
            return None

    def _glob_paths(self, pattern: str) -> List[str]:
        """Expand glob patterns in path."""
        import glob
        try:
            return glob.glob(pattern)
        except Exception:
            return []

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
            with open(self.cache_path, "r", encoding="utf-8") as f:
                data = json.load(f)

            for agent_name, info_data in data.get("agents", {}).items():
                self._cache[agent_name] = AgentInfo.from_dict(info_data)
        except (json.JSONDecodeError, KeyError, TypeError):
            # Invalid cache, start fresh
            self._cache = {}

    def _save_cache(self) -> None:
        """Save cache to disk."""
        data = {
            "version": "1.0.0",
            "updated_at": datetime.now().isoformat(),
            "platform": self.detect_platform().value,
            "agents": {
                name: info.to_dict()
                for name, info in self._cache.items()
            },
        }

        try:
            with open(self.cache_path, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=2)
        except OSError:
            pass  # Cache write failure is non-critical

    def clear_cache(self) -> None:
        """Clear the detection cache."""
        self._cache = {}
        if self.cache_path.exists():
            try:
                self.cache_path.unlink()
            except OSError:
                pass

    def get_detection_summary(self) -> Dict[str, Any]:
        """Get a summary of all detected agents."""
        all_agents = self.detect_all_agents()
        available = [info for info in all_agents.values() if info.available]
        unavailable = [info for info in all_agents.values() if not info.available]

        return {
            "platform": self.detect_platform().value,
            "available_count": len(available),
            "unavailable_count": len(unavailable),
            "available": [
                {
                    "name": info.name,
                    "path": info.path,
                    "version": info.version,
                    "method": info.detection_method,
                }
                for info in available
            ],
            "unavailable": [info.name for info in unavailable],
            "cache_path": str(self.cache_path),
        }
