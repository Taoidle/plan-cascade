"""
Interactive configuration wizard for Plan Cascade.

This module provides a CLI-based setup wizard using the rich library
for formatted output and user interaction.
"""

from typing import Optional

from rich.console import Console
from rich.panel import Panel
from rich.prompt import Confirm, Prompt
from rich.table import Table

from .models import BackendType, Settings
from .storage import SettingsStorage
from .validation import ConfigValidator


class SetupWizard:
    """
    Interactive setup wizard for Plan Cascade configuration.

    Guides users through initial configuration including:
    - Backend selection
    - API key configuration (when needed)
    - Configuration confirmation and save

    Uses the rich library for formatted terminal output.

    Attributes:
        console: Rich Console instance for output.
        storage: SettingsStorage instance for persistence.
        validator: ConfigValidator for validation checks.
    """

    # Backend options mapping
    BACKEND_OPTIONS = {
        "1": (BackendType.CLAUDE_CODE, "Claude Code", "Recommended - No API Key required"),
        "2": (BackendType.CLAUDE_API, "Claude API", "Anthropic Claude API"),
        "3": (BackendType.OPENAI, "OpenAI", "OpenAI GPT models"),
        "4": (BackendType.DEEPSEEK, "DeepSeek", "DeepSeek API"),
        "5": (BackendType.OLLAMA, "Ollama", "Local Ollama instance"),
    }

    # Provider names for API key lookup
    BACKEND_TO_PROVIDER = {
        BackendType.CLAUDE_API: "claude",
        BackendType.OPENAI: "openai",
        BackendType.DEEPSEEK: "deepseek",
        BackendType.OLLAMA: "ollama",
    }

    def __init__(
        self,
        storage: Optional[SettingsStorage] = None,
        console: Optional[Console] = None,
    ) -> None:
        """
        Initialize the setup wizard.

        Args:
            storage: SettingsStorage instance. Creates default if not provided.
            console: Rich Console instance. Creates default if not provided.
        """
        self.console = console or Console()
        self.storage = storage or SettingsStorage()
        self.validator = ConfigValidator()

    def run(self) -> Settings:
        """
        Run the complete configuration wizard.

        Workflow:
        1. Display welcome message
        2. Backend selection
        3. API key configuration (if needed)
        4. Configuration summary and confirmation
        5. Save configuration

        Returns:
            Configured Settings object.
        """
        self._display_welcome()

        # Load existing settings as base
        settings = self.storage.load()

        # Step 1: Backend selection
        backend = self.run_backend_setup()
        settings.backend = backend

        # Update provider based on backend
        if backend in self.BACKEND_TO_PROVIDER:
            settings.provider = self.BACKEND_TO_PROVIDER[backend]

        # Step 2: API key configuration (if needed)
        if backend != BackendType.CLAUDE_CODE:
            self.run_api_key_setup(settings.provider)

        # Step 3: Confirm and save
        self._display_summary(settings)

        if Confirm.ask("\nSave this configuration?", default=True):
            self.storage.save(settings)
            self.console.print("\n[green]Configuration saved successfully![/green]")
        else:
            self.console.print("\n[yellow]Configuration not saved.[/yellow]")

        return settings

    def run_backend_setup(self) -> BackendType:
        """
        Run the backend selection wizard step.

        Displays available backends and prompts user to select one.

        Returns:
            Selected BackendType.
        """
        self.console.print("\n[bold]Step 1: Select Backend[/bold]\n")

        # Create options table
        table = Table(show_header=False, box=None, padding=(0, 2))
        table.add_column("Choice", style="cyan")
        table.add_column("Name", style="white")
        table.add_column("Description", style="dim")

        for choice, (_, name, description) in self.BACKEND_OPTIONS.items():
            table.add_row(f"  {choice}.", name, description)

        self.console.print(table)
        self.console.print()

        # Get user choice
        choice = Prompt.ask(
            "Select backend",
            choices=list(self.BACKEND_OPTIONS.keys()),
            default="1",
        )

        backend, name, _ = self.BACKEND_OPTIONS[choice]
        self.console.print(f"\n[green]Selected: {name}[/green]")

        return backend

    def run_api_key_setup(self, provider: str) -> None:
        """
        Run the API key configuration wizard step.

        Prompts for API key input using password mode (hidden input).

        Args:
            provider: Provider name for the API key.
        """
        self.console.print(f"\n[bold]Step 2: Configure API Key for {provider}[/bold]\n")

        # Check if API key already exists
        if self.storage.has_api_key(provider):
            self.console.print(f"[dim]An API key for {provider} is already stored.[/dim]")
            if not Confirm.ask("Update the API key?", default=False):
                return

        # Prompt for API key (password mode hides input)
        api_key = Prompt.ask(
            f"Enter {provider} API Key",
            password=True,
        )

        if api_key:
            self.storage.set_api_key(provider, api_key)
            self.console.print(f"[green]API key for {provider} stored securely.[/green]")
        else:
            self.console.print("[yellow]No API key provided.[/yellow]")

    def _display_welcome(self) -> None:
        """Display the welcome message panel."""
        welcome_text = (
            "[bold]Plan Cascade Configuration Wizard[/bold]\n\n"
            "This wizard will help you configure Plan Cascade.\n"
            "You can re-run this wizard anytime with [cyan]plan-cascade config --setup[/cyan]"
        )
        self.console.print(Panel(welcome_text, title="Welcome", border_style="blue"))

    def _display_summary(self, settings: Settings) -> None:
        """
        Display configuration summary.

        Args:
            settings: Settings object to summarize.
        """
        self.console.print("\n[bold]Configuration Summary[/bold]\n")

        table = Table(show_header=False, box=None)
        table.add_column("Setting", style="cyan")
        table.add_column("Value", style="white")

        table.add_row("Backend", settings.backend.value)
        table.add_row("Provider", settings.provider)
        table.add_row("Model", settings.model or "(default)")

        # Check API key status
        if settings.backend != BackendType.CLAUDE_CODE:
            provider = self.BACKEND_TO_PROVIDER.get(settings.backend, settings.provider)
            has_key = self.storage.has_api_key(provider)
            key_status = "[green]Configured[/green]" if has_key else "[red]Not set[/red]"
            table.add_row("API Key", key_status)

        table.add_row("Default Mode", settings.default_mode)
        table.add_row("Theme", settings.theme)

        self.console.print(table)

        # Validate and show any issues
        result = self.validator.validate(settings)
        cred_result = self.validator.validate_credentials(settings, self.storage)
        result.merge(cred_result)

        if result.errors:
            self.console.print("\n[red]Errors:[/red]")
            for error in result.errors:
                self.console.print(f"  [red]- {error}[/red]")

        if result.warnings:
            self.console.print("\n[yellow]Warnings:[/yellow]")
            for warning in result.warnings:
                self.console.print(f"  [yellow]- {warning}[/yellow]")


def run_setup_wizard(
    storage: Optional[SettingsStorage] = None,
    console: Optional[Console] = None,
) -> Settings:
    """
    Convenience function to run the setup wizard.

    Args:
        storage: Optional SettingsStorage instance.
        console: Optional Rich Console instance.

    Returns:
        Configured Settings object.
    """
    wizard = SetupWizard(storage=storage, console=console)
    return wizard.run()
