"""Tests for ConfigManager module."""

import json
import os
import sys
from pathlib import Path
from unittest.mock import patch

import pytest

from plan_cascade.state.config_manager import ConfigManager


class TestConfigManagerBasic:
    """Basic tests for ConfigManager class."""

    def test_init_default(self, tmp_path: Path):
        """Test ConfigManager initialization with default project root."""
        with patch.object(Path, "cwd", return_value=tmp_path):
            config = ConfigManager()
            assert config.project_root == tmp_path.resolve()

    def test_init_with_project_root(self, tmp_path: Path):
        """Test ConfigManager initialization with custom project root."""
        config = ConfigManager(tmp_path)
        assert config.project_root == tmp_path.resolve()


class TestConfigGet:
    """Tests for configuration value retrieval."""

    def test_get_default_value(self, tmp_path: Path):
        """Test getting a value returns default when not configured."""
        config = ConfigManager(tmp_path)
        value = config.get("legacy_mode")
        assert value is False  # Built-in default

    def test_get_with_provided_default(self, tmp_path: Path):
        """Test getting a value with provided default."""
        config = ConfigManager(tmp_path)
        value = config.get("nonexistent_key", default="my_default")
        assert value == "my_default"

    def test_get_nested_default(self, tmp_path: Path):
        """Test getting nested value from defaults."""
        config = ConfigManager(tmp_path)
        value = config.get("quality_gates.test")
        assert value is True

    def test_get_from_global_config(self, tmp_path: Path):
        """Test getting value from global config file."""
        # Create global config
        global_config_dir = tmp_path / ".plan-cascade"
        global_config_dir.mkdir()
        global_config_file = global_config_dir / "config.json"
        global_config_file.write_text(json.dumps({"max_retries": 10}))

        with patch.object(ConfigManager, "_get_global_config_path", return_value=global_config_file):
            config = ConfigManager(tmp_path)
            value = config.get("max_retries")
            assert value == 10

    def test_get_from_project_config(self, tmp_path: Path):
        """Test getting value from project config file."""
        # Create project config
        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({"max_retries": 7}))

        config = ConfigManager(tmp_path)
        value = config.get("max_retries")
        assert value == 7

    def test_get_env_var_priority(self, tmp_path: Path):
        """Test that environment variable has highest priority."""
        # Create configs with different values
        global_config_dir = tmp_path / ".plan-cascade"
        global_config_dir.mkdir()
        global_config_file = global_config_dir / "config.json"
        global_config_file.write_text(json.dumps({"legacy_mode": False}))

        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({"legacy_mode": False}))

        with patch.object(ConfigManager, "_get_global_config_path", return_value=global_config_file):
            with patch.dict(os.environ, {"PLAN_CASCADE_LEGACY_MODE": "1"}):
                config = ConfigManager(tmp_path)
                value = config.get("legacy_mode")
                assert value is True

    def test_get_project_config_over_global(self, tmp_path: Path):
        """Test that project config has priority over global config."""
        global_config_dir = tmp_path / ".plan-cascade"
        global_config_dir.mkdir()
        global_config_file = global_config_dir / "config.json"
        global_config_file.write_text(json.dumps({"max_retries": 10}))

        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({"max_retries": 5}))

        with patch.object(ConfigManager, "_get_global_config_path", return_value=global_config_file):
            config = ConfigManager(tmp_path)
            value = config.get("max_retries")
            assert value == 5


class TestConfigSet:
    """Tests for configuration value setting."""

    def test_set_global_config(self, tmp_path: Path):
        """Test setting a value in global config."""
        global_config_file = tmp_path / "global-config.json"

        with patch.object(ConfigManager, "_get_global_config_path", return_value=global_config_file):
            config = ConfigManager(tmp_path)
            config.set("max_retries", 15, scope="global")

            # Verify file was written
            assert global_config_file.exists()
            data = json.loads(global_config_file.read_text())
            assert data["max_retries"] == 15

    def test_set_project_config(self, tmp_path: Path):
        """Test setting a value in project config."""
        config = ConfigManager(tmp_path)
        config.set("max_retries", 8, scope="project")

        # Verify file was written
        project_config_file = tmp_path / ".plan-cascade.json"
        assert project_config_file.exists()
        data = json.loads(project_config_file.read_text())
        assert data["max_retries"] == 8

    def test_set_nested_value(self, tmp_path: Path):
        """Test setting a nested configuration value."""
        config = ConfigManager(tmp_path)
        config.set("quality_gates.test", False, scope="project")

        project_config_file = tmp_path / ".plan-cascade.json"
        data = json.loads(project_config_file.read_text())
        assert data["quality_gates"]["test"] is False

    def test_set_invalid_scope(self, tmp_path: Path):
        """Test that invalid scope raises ValueError."""
        config = ConfigManager(tmp_path)
        with pytest.raises(ValueError, match="Invalid scope"):
            config.set("key", "value", scope="invalid")


class TestConfigReset:
    """Tests for configuration reset functionality."""

    def test_reset_global_config(self, tmp_path: Path):
        """Test resetting entire global config."""
        global_config_file = tmp_path / "global-config.json"
        global_config_file.write_text(json.dumps({"max_retries": 10}))

        with patch.object(ConfigManager, "_get_global_config_path", return_value=global_config_file):
            config = ConfigManager(tmp_path)
            config.reset(scope="global")

            assert not global_config_file.exists()

    def test_reset_project_config(self, tmp_path: Path):
        """Test resetting entire project config."""
        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({"max_retries": 10}))

        config = ConfigManager(tmp_path)
        config.reset(scope="project")

        assert not project_config_file.exists()

    def test_reset_specific_key(self, tmp_path: Path):
        """Test resetting a specific configuration key."""
        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({
            "max_retries": 10,
            "timeout_seconds": 600
        }))

        config = ConfigManager(tmp_path)
        config.reset(key="max_retries", scope="project")

        data = json.loads(project_config_file.read_text())
        assert "max_retries" not in data
        assert data["timeout_seconds"] == 600

    def test_reset_invalid_scope(self, tmp_path: Path):
        """Test that invalid scope raises ValueError."""
        config = ConfigManager(tmp_path)
        with pytest.raises(ValueError, match="Invalid scope"):
            config.reset(scope="invalid")


class TestGetDataDir:
    """Tests for data directory resolution."""

    def test_get_data_dir_env_override(self, tmp_path: Path):
        """Test data directory from environment variable."""
        custom_dir = tmp_path / "custom-data"

        with patch.dict(os.environ, {"PLAN_CASCADE_DATA_DIR": str(custom_dir)}):
            config = ConfigManager(tmp_path)
            data_dir = config.get_data_dir()
            assert data_dir == custom_dir

    def test_get_data_dir_project_config(self, tmp_path: Path):
        """Test data directory from project config."""
        custom_dir = tmp_path / "project-data"
        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({"data_dir": str(custom_dir)}))

        config = ConfigManager(tmp_path)
        data_dir = config.get_data_dir()
        assert data_dir == custom_dir

    def test_get_data_dir_global_config(self, tmp_path: Path):
        """Test data directory from global config."""
        custom_dir = tmp_path / "global-data"
        global_config_file = tmp_path / "global-config.json"
        global_config_file.write_text(json.dumps({"data_dir": str(custom_dir)}))

        with patch.object(ConfigManager, "_get_global_config_path", return_value=global_config_file):
            config = ConfigManager(tmp_path)
            data_dir = config.get_data_dir()
            assert data_dir == custom_dir

    @patch("sys.platform", "win32")
    @patch.dict("os.environ", {"APPDATA": "C:\\Users\\Test\\AppData\\Roaming"})
    def test_get_data_dir_windows_default(self, tmp_path: Path):
        """Test default data directory on Windows."""
        config = ConfigManager(tmp_path)
        data_dir = config.get_data_dir()
        assert "plan-cascade" in str(data_dir)

    @patch("sys.platform", "linux")
    def test_get_data_dir_unix_default(self, tmp_path: Path):
        """Test default data directory on Unix."""
        config = ConfigManager(tmp_path)
        data_dir = config.get_data_dir()
        assert ".plan-cascade" in str(data_dir)

    def test_get_data_dir_env_over_config(self, tmp_path: Path):
        """Test that environment variable has priority over config."""
        env_dir = tmp_path / "env-data"
        config_dir = tmp_path / "config-data"

        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({"data_dir": str(config_dir)}))

        with patch.dict(os.environ, {"PLAN_CASCADE_DATA_DIR": str(env_dir)}):
            config = ConfigManager(tmp_path)
            data_dir = config.get_data_dir()
            assert data_dir == env_dir


class TestIsLegacyMode:
    """Tests for legacy mode detection."""

    def test_is_legacy_mode_env_true(self, tmp_path: Path):
        """Test legacy mode from environment variable (true)."""
        with patch.dict(os.environ, {"PLAN_CASCADE_LEGACY_MODE": "1"}):
            config = ConfigManager(tmp_path)
            assert config.is_legacy_mode() is True

    def test_is_legacy_mode_env_false(self, tmp_path: Path):
        """Test legacy mode from environment variable (false)."""
        with patch.dict(os.environ, {"PLAN_CASCADE_LEGACY_MODE": "0"}):
            config = ConfigManager(tmp_path)
            assert config.is_legacy_mode() is False

    def test_is_legacy_mode_env_variants(self, tmp_path: Path):
        """Test legacy mode with various true values."""
        for value in ["true", "True", "TRUE", "yes", "YES", "on", "ON", "1"]:
            with patch.dict(os.environ, {"PLAN_CASCADE_LEGACY_MODE": value}, clear=False):
                config = ConfigManager(tmp_path)
                # Clear cache
                config._global_config = None
                config._project_config = None
                assert config.is_legacy_mode() is True, f"Failed for value: {value}"

    def test_is_legacy_mode_project_config(self, tmp_path: Path):
        """Test legacy mode from project config."""
        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({"legacy_mode": True}))

        config = ConfigManager(tmp_path)
        assert config.is_legacy_mode() is True

    def test_is_legacy_mode_global_config(self, tmp_path: Path):
        """Test legacy mode from global config."""
        global_config_file = tmp_path / "global-config.json"
        global_config_file.write_text(json.dumps({"legacy_mode": True}))

        with patch.object(ConfigManager, "_get_global_config_path", return_value=global_config_file):
            config = ConfigManager(tmp_path)
            assert config.is_legacy_mode() is True

    def test_is_legacy_mode_auto_detect(self, tmp_path: Path):
        """Test legacy mode auto-detection with existing prd.json."""
        # Create prd.json in project root
        prd_file = tmp_path / "prd.json"
        prd_file.write_text(json.dumps({"stories": []}))

        config = ConfigManager(tmp_path)
        assert config.is_legacy_mode() is True

    def test_is_legacy_mode_default_false(self, tmp_path: Path):
        """Test legacy mode defaults to False."""
        config = ConfigManager(tmp_path)
        assert config.is_legacy_mode() is False

    def test_is_legacy_mode_env_over_config(self, tmp_path: Path):
        """Test environment variable overrides config."""
        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({"legacy_mode": True}))

        with patch.dict(os.environ, {"PLAN_CASCADE_LEGACY_MODE": "0"}):
            config = ConfigManager(tmp_path)
            assert config.is_legacy_mode() is False


class TestGetAll:
    """Tests for getting all configuration values."""

    def test_get_all_defaults(self, tmp_path: Path):
        """Test get_all returns defaults when no config exists."""
        config = ConfigManager(tmp_path)
        all_config = config.get_all()

        assert all_config["legacy_mode"] is False
        assert all_config["max_retries"] == 5
        assert all_config["quality_gates"]["test"] is True

    def test_get_all_merged(self, tmp_path: Path):
        """Test get_all merges all config sources."""
        global_config_file = tmp_path / "global-config.json"
        global_config_file.write_text(json.dumps({"max_retries": 10}))

        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({"timeout_seconds": 600}))

        with patch.object(ConfigManager, "_get_global_config_path", return_value=global_config_file):
            config = ConfigManager(tmp_path)
            all_config = config.get_all()

            assert all_config["max_retries"] == 10
            assert all_config["timeout_seconds"] == 600


class TestReload:
    """Tests for configuration reload functionality."""

    def test_reload_clears_cache(self, tmp_path: Path):
        """Test reload clears cached configuration."""
        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({"max_retries": 5}))

        config = ConfigManager(tmp_path)
        assert config.get("max_retries") == 5

        # Modify the file
        project_config_file.write_text(json.dumps({"max_retries": 10}))

        # Without reload, should still see old value (cached)
        assert config.get("max_retries") == 5

        # After reload, should see new value
        config.reload()
        assert config.get("max_retries") == 10


class TestNestedConfig:
    """Tests for nested configuration handling."""

    def test_get_deeply_nested(self, tmp_path: Path):
        """Test getting deeply nested configuration values."""
        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({
            "level1": {
                "level2": {
                    "level3": "deep_value"
                }
            }
        }))

        config = ConfigManager(tmp_path)
        value = config.get("level1.level2.level3")
        assert value == "deep_value"

    def test_set_deeply_nested(self, tmp_path: Path):
        """Test setting deeply nested configuration values."""
        config = ConfigManager(tmp_path)
        config.set("level1.level2.level3", "deep_value", scope="project")

        project_config_file = tmp_path / ".plan-cascade.json"
        data = json.loads(project_config_file.read_text())
        assert data["level1"]["level2"]["level3"] == "deep_value"

    def test_delete_nested(self, tmp_path: Path):
        """Test deleting nested configuration keys."""
        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text(json.dumps({
            "quality_gates": {
                "test": True,
                "lint": True
            }
        }))

        config = ConfigManager(tmp_path)
        config.reset(key="quality_gates.test", scope="project")

        data = json.loads(project_config_file.read_text())
        assert "test" not in data["quality_gates"]
        assert data["quality_gates"]["lint"] is True


class TestConfigFilePaths:
    """Tests for configuration file path resolution."""

    @patch("sys.platform", "win32")
    @patch.dict("os.environ", {"APPDATA": "C:\\Users\\Test\\AppData\\Roaming"})
    def test_global_config_path_windows(self, tmp_path: Path):
        """Test global config path on Windows."""
        config = ConfigManager(tmp_path)
        path = config._get_global_config_path()
        assert "plan-cascade" in str(path)
        assert "config.json" in str(path)

    @patch("sys.platform", "linux")
    def test_global_config_path_unix(self, tmp_path: Path):
        """Test global config path on Unix."""
        config = ConfigManager(tmp_path)
        path = config._get_global_config_path()
        assert ".plan-cascade" in str(path)
        assert "config.json" in str(path)

    def test_project_config_path(self, tmp_path: Path):
        """Test project config path."""
        config = ConfigManager(tmp_path)
        path = config._get_project_config_path()
        assert path == tmp_path / ".plan-cascade.json"


class TestEnvVarParsing:
    """Tests for environment variable parsing."""

    def test_env_bool_parsing(self, tmp_path: Path):
        """Test boolean parsing from environment variables."""
        config = ConfigManager(tmp_path)

        # Test various true values
        for value in ["1", "true", "yes", "on"]:
            with patch.dict(os.environ, {"PLAN_CASCADE_LEGACY_MODE": value}):
                config._global_config = None  # Clear cache
                config._project_config = None
                assert config._get_env_value("legacy_mode") is True

        # Test false values
        for value in ["0", "false", "no", "off"]:
            with patch.dict(os.environ, {"PLAN_CASCADE_LEGACY_MODE": value}):
                config._global_config = None  # Clear cache
                config._project_config = None
                assert config._get_env_value("legacy_mode") is False

    def test_env_int_parsing(self, tmp_path: Path):
        """Test integer parsing from environment variables."""
        config = ConfigManager(tmp_path)

        with patch.dict(os.environ, {"PLAN_CASCADE_MAX_PARALLEL_STORIES": "5"}):
            value = config._get_env_value("max_parallel_stories")
            assert value == 5
            assert isinstance(value, int)

    def test_env_invalid_int(self, tmp_path: Path):
        """Test invalid integer returns None."""
        config = ConfigManager(tmp_path)

        with patch.dict(os.environ, {"PLAN_CASCADE_MAX_RETRIES": "not_a_number"}):
            value = config._get_env_value("max_retries")
            assert value is None


class TestConfigFileErrors:
    """Tests for handling configuration file errors."""

    def test_corrupted_global_config(self, tmp_path: Path):
        """Test handling corrupted global config file."""
        global_config_file = tmp_path / "global-config.json"
        global_config_file.write_text("not valid json {")

        with patch.object(ConfigManager, "_get_global_config_path", return_value=global_config_file):
            config = ConfigManager(tmp_path)
            # Should return default, not crash
            value = config.get("max_retries")
            assert value == 5  # Default value

    def test_corrupted_project_config(self, tmp_path: Path):
        """Test handling corrupted project config file."""
        project_config_file = tmp_path / ".plan-cascade.json"
        project_config_file.write_text("{ invalid json")

        config = ConfigManager(tmp_path)
        # Should return default, not crash
        value = config.get("max_retries")
        assert value == 5  # Default value

    def test_missing_config_files(self, tmp_path: Path):
        """Test behavior when config files don't exist."""
        config = ConfigManager(tmp_path)
        # Should work fine with defaults
        assert config.get("max_retries") == 5
        assert config.is_legacy_mode() is False
