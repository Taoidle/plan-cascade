"""
Backend Factory

Factory pattern for creating backend instances based on configuration.
Supports all backend types and provides configuration validation.
"""

from pathlib import Path
from typing import Any, Dict, List, Optional, Type

from .base import AgentBackend, ExecutionResult


# Type alias for backend classes
BackendClass = Type[AgentBackend]


class BackendFactory:
    """
    Factory for creating backend instances.

    Supports:
    - claude-code: Claude Code CLI backend (no API key needed)
    - builtin: Direct LLM API with ReAct loop
    - External backends (aider, codex, amp) via CLI

    Example:
        # Create Claude Code backend
        backend = BackendFactory.create({"backend": "claude-code"})

        # Create builtin backend with Claude
        backend = BackendFactory.create({
            "backend": "builtin",
            "provider": "claude",
            "api_key": "sk-ant-...",
        })

        # Create from settings
        backend = BackendFactory.create_from_settings(settings)
    """

    # Registry of backend classes
    _backends: Dict[str, BackendClass] = {}

    # Default configuration for backends
    _default_configs: Dict[str, Dict[str, Any]] = {
        "claude-code": {
            "claude_path": "claude",
            "output_format": "stream-json",
            "print_mode": "tools",
        },
        "builtin": {
            "provider": "claude",
            "max_iterations": 50,
        },
    }

    @classmethod
    def register(cls, name: str, backend_class: BackendClass) -> None:
        """
        Register a backend class.

        Args:
            name: Backend name
            backend_class: Backend class to register
        """
        cls._backends[name.lower()] = backend_class

    @classmethod
    def unregister(cls, name: str) -> None:
        """
        Unregister a backend.

        Args:
            name: Backend name to unregister
        """
        cls._backends.pop(name.lower(), None)

    @classmethod
    def create(cls, config: Dict[str, Any]) -> AgentBackend:
        """
        Create a backend instance from configuration.

        Args:
            config: Configuration dictionary with at least:
                - backend: Backend type ("claude-code", "builtin", etc.)
                And for builtin:
                - provider: LLM provider ("claude", "openai", "ollama")
                - api_key: API key (optional for claude-code and ollama)
                - model: Model identifier (optional)

        Returns:
            AgentBackend instance

        Raises:
            ValueError: If backend type is unknown or configuration is invalid
        """
        backend_type = config.get("backend", "claude-code").lower()

        # Apply default configuration
        merged_config = cls._default_configs.get(backend_type, {}).copy()
        merged_config.update(config)

        # Get project root
        project_root = merged_config.get("project_root")
        if project_root:
            project_root = Path(project_root)

        # Get backend class
        backend_class = cls._get_backend_class(backend_type)

        # Create instance based on backend type
        if backend_type == "claude-code":
            return backend_class(
                claude_path=merged_config.get("claude_path", "claude"),
                project_root=project_root,
                output_format=merged_config.get("output_format", "stream-json"),
                print_mode=merged_config.get("print_mode", "tools"),
            )

        elif backend_type == "builtin":
            return backend_class(
                provider=merged_config.get("provider", "claude"),
                model=merged_config.get("model"),
                api_key=merged_config.get("api_key"),
                base_url=merged_config.get("base_url"),
                max_iterations=merged_config.get("max_iterations", 50),
                project_root=project_root,
                config=merged_config,
            )

        elif backend_type in ("aider", "codex", "amp", "cursor"):
            # External CLI backends
            from .external import ExternalCLIBackend
            return ExternalCLIBackend(
                backend_type=backend_type,
                config=merged_config,
                project_root=project_root,
            )

        else:
            # Try registered custom backends
            if backend_type in cls._backends:
                return cls._backends[backend_type](
                    project_root=project_root,
                    **merged_config
                )

            raise ValueError(
                f"Unknown backend: {backend_type}. "
                f"Supported: {cls.get_supported_backends()}"
            )

    @classmethod
    def _get_backend_class(cls, backend_type: str) -> BackendClass:
        """
        Get the backend class for a backend type.

        Args:
            backend_type: Backend type name

        Returns:
            Backend class

        Raises:
            ValueError: If backend type is unknown
        """
        # Check registry first
        if backend_type in cls._backends:
            return cls._backends[backend_type]

        # Lazy import built-in backends
        if backend_type == "claude-code":
            from .claude_code import ClaudeCodeBackend
            cls._backends["claude-code"] = ClaudeCodeBackend
            return ClaudeCodeBackend

        elif backend_type == "builtin":
            from .builtin import BuiltinBackend
            cls._backends["builtin"] = BuiltinBackend
            return BuiltinBackend

        raise ValueError(f"Unknown backend type: {backend_type}")

    @classmethod
    def get_supported_backends(cls) -> List[str]:
        """
        Get list of supported backend types.

        Returns:
            List of backend type names
        """
        built_in = ["claude-code", "builtin", "aider", "codex", "amp", "cursor"]
        registered = list(cls._backends.keys())
        return list(set(built_in + registered))

    @classmethod
    def get_default_config(cls, backend_type: str) -> Dict[str, Any]:
        """
        Get default configuration for a backend.

        Args:
            backend_type: Backend type

        Returns:
            Default configuration dictionary
        """
        return cls._default_configs.get(backend_type.lower(), {}).copy()

    @classmethod
    def set_default_config(cls, backend_type: str, config: Dict[str, Any]) -> None:
        """
        Set default configuration for a backend.

        Args:
            backend_type: Backend type
            config: Configuration dictionary
        """
        cls._default_configs[backend_type.lower()] = config

    @classmethod
    def create_from_settings(cls, settings: Any) -> AgentBackend:
        """
        Create a backend from a Settings object.

        Args:
            settings: Settings object with backend, provider, model attributes

        Returns:
            AgentBackend instance
        """
        # Build config from settings
        config: Dict[str, Any] = {}

        # Get backend type
        backend = getattr(settings, "backend", "claude-code")
        if hasattr(backend, "value"):  # Enum
            backend = backend.value
        config["backend"] = backend

        # Get provider for builtin
        if backend == "builtin" or backend in ("claude-api", "openai", "deepseek", "ollama"):
            provider = getattr(settings, "provider", None)
            if provider:
                config["provider"] = provider

            # Map backend types to providers
            backend_to_provider = {
                "claude-api": "claude",
                "openai": "openai",
                "deepseek": "openai",  # DeepSeek uses OpenAI-compatible API
                "ollama": "ollama",
            }
            if backend in backend_to_provider:
                config["provider"] = backend_to_provider[backend]
                config["backend"] = "builtin"

        # Get model
        model = getattr(settings, "model", None)
        if model:
            config["model"] = model

        # Get API key (may come from settings or keyring)
        api_key = None
        if hasattr(settings, "get_api_key"):
            api_key = settings.get_api_key()
        elif hasattr(settings, "api_key"):
            api_key = settings.api_key
        if api_key:
            config["api_key"] = api_key

        # Get other settings
        if hasattr(settings, "max_iterations"):
            config["max_iterations"] = settings.max_iterations

        return cls.create(config)

    @classmethod
    def validate_config(cls, config: Dict[str, Any]) -> List[str]:
        """
        Validate a configuration dictionary.

        Args:
            config: Configuration to validate

        Returns:
            List of validation error messages (empty if valid)
        """
        errors = []

        backend_type = config.get("backend", "claude-code").lower()

        # Check if backend is supported
        if backend_type not in cls.get_supported_backends():
            errors.append(f"Unknown backend: {backend_type}")
            return errors

        # Validate builtin-specific config
        if backend_type == "builtin":
            provider = config.get("provider", "claude")

            # Check API key requirement
            if provider in ("claude", "openai", "deepseek"):
                if not config.get("api_key"):
                    errors.append(
                        f"API key required for {provider} provider. "
                        f"Set api_key in config or use environment variable."
                    )

            # Check provider is valid
            valid_providers = ["claude", "openai", "ollama", "deepseek"]
            if provider not in valid_providers:
                errors.append(
                    f"Unknown provider: {provider}. "
                    f"Valid providers: {valid_providers}"
                )

        return errors


# External CLI Backend for aider, codex, etc.
class ExternalCLIBackend(AgentBackend):
    """
    Backend for external CLI tools like aider, codex, amp.

    Executes tasks by running external CLI commands.
    """

    # Default configurations for external tools
    TOOL_CONFIGS = {
        "aider": {
            "command": "aider",
            "args": ["--yes", "--no-git", "-m", "{prompt}"],
        },
        "codex": {
            "command": "codex",
            "args": ["-m", "{prompt}"],
        },
        "amp": {
            "command": "amp-code",
            "args": ["--prompt", "{prompt}"],
        },
        "cursor": {
            "command": "cursor-cli",
            "args": ["--prompt", "{prompt}"],
        },
    }

    def __init__(
        self,
        backend_type: str,
        config: Optional[Dict[str, Any]] = None,
        project_root: Optional[Path] = None
    ):
        """
        Initialize external CLI backend.

        Args:
            backend_type: Type of external tool
            config: Configuration dictionary
            project_root: Project root directory
        """
        super().__init__(project_root)

        self.backend_type = backend_type.lower()
        self.config = config or {}

        # Get tool configuration
        tool_config = self.TOOL_CONFIGS.get(self.backend_type, {})
        self.command = self.config.get("command", tool_config.get("command", backend_type))
        self.args_template = self.config.get("args", tool_config.get("args", []))

        self._process = None

    async def execute(
        self,
        story: Dict[str, Any],
        context: str = ""
    ) -> ExecutionResult:
        """Execute using external CLI tool."""
        import asyncio
        import shutil

        story_id = story.get("id", "unknown")
        prompt = self._build_prompt(story, context)

        # Check if tool is available
        if not shutil.which(self.command):
            return ExecutionResult(
                success=False,
                error=f"External tool not found: {self.command}",
                story_id=story_id,
                agent=self.backend_type,
            )

        # Build command
        args = []
        for arg in self.args_template:
            if isinstance(arg, str):
                arg = arg.replace("{prompt}", prompt)
                arg = arg.replace("{story_id}", story_id)
            args.append(arg)

        cmd = [self.command] + args

        try:
            process = await asyncio.create_subprocess_exec(
                *cmd,
                cwd=str(self.project_root),
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.STDOUT,
            )

            self._process = process
            stdout, _ = await process.communicate()

            output = stdout.decode("utf-8", errors="replace") if stdout else ""
            success = process.returncode == 0

            return ExecutionResult(
                success=success,
                output=output,
                iterations=1,
                error=None if success else f"Command exited with code {process.returncode}",
                story_id=story_id,
                agent=self.backend_type,
                metadata={"exit_code": process.returncode}
            )

        except Exception as e:
            return ExecutionResult(
                success=False,
                error=str(e),
                story_id=story_id,
                agent=self.backend_type,
            )
        finally:
            self._process = None

    def get_llm(self):
        """Get LLM provider (not available for external backends)."""
        raise NotImplementedError(
            f"External backend '{self.backend_type}' does not provide LLM access. "
            "Use a different backend for PRD generation."
        )

    def get_name(self) -> str:
        """Get backend name."""
        return self.backend_type
