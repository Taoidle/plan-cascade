"""Tests for the Dependencies CLI command."""

import json
import pytest
from pathlib import Path

from src.plan_cascade.cli.dependencies import (
    DependencyGraphAnalyzer,
    DependencyGraphResult,
    DependencyNode,
    OutputFormat,
)
from src.plan_cascade.state.path_resolver import PathResolver

try:
    import typer
    from typer.testing import CliRunner
    from src.plan_cascade.cli.dependencies import dependencies_command

    # Create a test app with the command as the default command (no subcommand name)
    test_app = typer.Typer()
    test_app.command()(dependencies_command)

    HAS_TYPER = True
    runner = CliRunner()
except ImportError:
    HAS_TYPER = False
    runner = None


class TestDependencyNode:
    """Tests for DependencyNode dataclass."""

    def test_node_creation(self):
        """Test creating a dependency node."""
        node = DependencyNode(
            id="story-001",
            title="Test Story",
            priority="high",
            status="pending",
            dependencies=["story-000"],
            dependents=["story-002"],
            depth=1,
            is_bottleneck=True,
            is_orphan=False,
            on_critical_path=True,
        )

        assert node.id == "story-001"
        assert node.title == "Test Story"
        assert node.depth == 1
        assert node.is_bottleneck is True
        assert node.on_critical_path is True

    def test_node_to_dict(self):
        """Test converting node to dictionary."""
        node = DependencyNode(
            id="story-001",
            title="Test",
            dependencies=["story-000"],
        )

        d = node.to_dict()
        assert d["id"] == "story-001"
        assert d["title"] == "Test"
        assert d["dependencies"] == ["story-000"]


class TestDependencyGraphAnalyzer:
    """Tests for DependencyGraphAnalyzer."""

    def test_analyze_prd(self, tmp_path, sample_prd):
        """Test analyzing a PRD file."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        analyzer = DependencyGraphAnalyzer(tmp_path)
        result = analyzer.analyze()

        assert result.source_type == "prd"
        assert len(result.nodes) == 4
        assert "story-001" in result.nodes
        assert "story-004" in result.nodes

    def test_analyze_mega_plan(self, tmp_path, sample_mega_plan):
        """Test analyzing a mega-plan file."""
        mega_path = tmp_path / "mega-plan.json"
        with open(mega_path, "w") as f:
            json.dump(sample_mega_plan, f)

        analyzer = DependencyGraphAnalyzer(tmp_path)
        result = analyzer.analyze()

        assert result.source_type == "mega-plan"
        assert len(result.nodes) == 3
        assert "feature-001" in result.nodes

    def test_mega_plan_takes_precedence(self, tmp_path, sample_prd, sample_mega_plan):
        """Test that mega-plan is analyzed when both exist."""
        prd_path = tmp_path / "prd.json"
        mega_path = tmp_path / "mega-plan.json"

        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)
        with open(mega_path, "w") as f:
            json.dump(sample_mega_plan, f)

        analyzer = DependencyGraphAnalyzer(tmp_path)
        result = analyzer.analyze()

        assert result.source_type == "mega-plan"

    def test_no_plan_raises_error(self, tmp_path):
        """Test that missing plan files raise an error."""
        analyzer = DependencyGraphAnalyzer(tmp_path)

        with pytest.raises(FileNotFoundError):
            analyzer.analyze()

    def test_finds_roots(self, tmp_path, sample_prd):
        """Test that root nodes are identified."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        analyzer = DependencyGraphAnalyzer(tmp_path)
        result = analyzer.analyze()

        assert "story-001" in result.roots
        assert "story-004" not in result.roots

    def test_calculates_dependents(self, tmp_path, sample_prd):
        """Test that dependents are calculated correctly."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        analyzer = DependencyGraphAnalyzer(tmp_path)
        result = analyzer.analyze()

        # story-001 should have story-002 and story-003 as dependents
        story_001 = result.nodes["story-001"]
        assert "story-002" in story_001.dependents
        assert "story-003" in story_001.dependents

    def test_calculates_depth(self, tmp_path, sample_prd):
        """Test that depth is calculated correctly."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        analyzer = DependencyGraphAnalyzer(tmp_path)
        result = analyzer.analyze()

        # story-001 is root -> depth 0
        assert result.nodes["story-001"].depth == 0
        # story-002 and story-003 depend on story-001 -> depth 1
        assert result.nodes["story-002"].depth == 1
        assert result.nodes["story-003"].depth == 1
        # story-004 depends on story-002 and story-003 -> depth 2
        assert result.nodes["story-004"].depth == 2

    def test_identifies_bottlenecks(self, tmp_path, sample_prd):
        """Test that bottlenecks are identified."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        analyzer = DependencyGraphAnalyzer(tmp_path)
        result = analyzer.analyze()

        # story-001 has 2 dependents, should be a bottleneck
        assert "story-001" in result.bottlenecks
        assert result.nodes["story-001"].is_bottleneck is True

    def test_identifies_orphans(self, tmp_path):
        """Test that orphan nodes are identified."""
        prd = {
            "metadata": {},
            "goal": "Test",
            "stories": [
                {
                    "id": "story-001",
                    "title": "Connected story",
                    "dependencies": [],
                    "status": "pending"
                },
                {
                    "id": "story-002",
                    "title": "Depends on 001",
                    "dependencies": ["story-001"],
                    "status": "pending"
                },
                {
                    "id": "story-orphan",
                    "title": "Orphan story",
                    "dependencies": [],
                    "status": "pending"
                }
            ]
        }

        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        analyzer = DependencyGraphAnalyzer(tmp_path)
        result = analyzer.analyze()

        # story-orphan has no deps and no dependents
        assert "story-orphan" in result.orphans
        assert result.nodes["story-orphan"].is_orphan is True
        # story-001 is not orphan (has dependents)
        assert "story-001" not in result.orphans

    def test_finds_critical_path(self, tmp_path, sample_prd):
        """Test that critical path is identified."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        analyzer = DependencyGraphAnalyzer(tmp_path)
        result = analyzer.analyze()

        # Critical path should be story-001 -> story-002/003 -> story-004
        assert len(result.critical_path) > 0
        assert result.critical_path[0] == "story-001"
        assert result.critical_path[-1] == "story-004"
        assert result.critical_path_length == 3

    def test_detects_circular_dependencies(self, tmp_path):
        """Test that circular dependencies are detected."""
        prd = {
            "metadata": {},
            "goal": "Test",
            "stories": [
                {
                    "id": "story-001",
                    "title": "Story 1",
                    "dependencies": ["story-003"],
                    "status": "pending"
                },
                {
                    "id": "story-002",
                    "title": "Story 2",
                    "dependencies": ["story-001"],
                    "status": "pending"
                },
                {
                    "id": "story-003",
                    "title": "Story 3",
                    "dependencies": ["story-002"],
                    "status": "pending"
                }
            ]
        }

        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        analyzer = DependencyGraphAnalyzer(tmp_path)
        result = analyzer.analyze()

        assert len(result.circular_dependencies) > 0
        assert result.has_issues is True


class TestDependencyGraphResult:
    """Tests for DependencyGraphResult."""

    def test_to_dict(self, tmp_path, sample_prd):
        """Test converting result to dictionary."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        analyzer = DependencyGraphAnalyzer(tmp_path)
        result = analyzer.analyze()
        d = result.to_dict()

        assert "source_type" in d
        assert "nodes" in d
        assert "roots" in d
        assert "critical_path" in d
        assert "bottlenecks" in d
        assert "orphans" in d
        assert "circular_dependencies" in d
        assert "summary" in d


@pytest.mark.skipif(not HAS_TYPER, reason="typer not available")
class TestDependenciesCLI:
    """Tests for dependencies CLI command."""

    def test_deps_requires_plan_file(self, tmp_path):
        """Test that 'deps' fails without a plan file."""
        result = runner.invoke(
            test_app,
            ["--project", str(tmp_path)]
        )

        assert result.exit_code == 1
        assert "no prd.json" in result.output.lower() or "not found" in result.output.lower()

    def test_deps_tree_format(self, tmp_path, sample_prd):
        """Test 'deps --format tree' output."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        result = runner.invoke(
            test_app,
            ["--project", str(tmp_path), "--format", "tree"]
        )

        assert result.exit_code == 0
        assert "story-001" in result.output

    def test_deps_flat_format(self, tmp_path, sample_prd):
        """Test 'deps --format flat' output."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        result = runner.invoke(
            test_app,
            ["--project", str(tmp_path), "--format", "flat"]
        )

        assert result.exit_code == 0
        assert "story-001" in result.output

    def test_deps_table_format(self, tmp_path, sample_prd):
        """Test 'deps --format table' output."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        result = runner.invoke(
            test_app,
            ["--project", str(tmp_path), "--format", "table"]
        )

        assert result.exit_code == 0
        assert "story-001" in result.output

    def test_deps_json_format(self, tmp_path, sample_prd):
        """Test 'deps --format json' output."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        result = runner.invoke(
            test_app,
            ["--project", str(tmp_path), "--format", "json"]
        )

        assert result.exit_code == 0
        # Should be valid JSON
        output_json = json.loads(result.output)
        assert "nodes" in output_json
        assert "critical_path" in output_json

    def test_deps_mega_plan(self, tmp_path, sample_mega_plan):
        """Test 'deps' with mega-plan."""
        mega_path = tmp_path / "mega-plan.json"
        with open(mega_path, "w") as f:
            json.dump(sample_mega_plan, f)

        result = runner.invoke(
            test_app,
            ["--project", str(tmp_path)]
        )

        assert result.exit_code == 0
        assert "feature-001" in result.output or "mega-plan" in result.output.lower()

    def test_deps_shows_critical_path(self, tmp_path, sample_prd):
        """Test that critical path is displayed."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        result = runner.invoke(
            test_app,
            ["--project", str(tmp_path), "--critical-path"]
        )

        assert result.exit_code == 0
        assert "critical path" in result.output.lower()

    def test_deps_no_critical_path_flag(self, tmp_path, sample_prd):
        """Test --no-critical-path flag."""
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        result = runner.invoke(
            test_app,
            ["--project", str(tmp_path), "--no-critical-path", "--no-check"]
        )

        assert result.exit_code == 0

    def test_deps_circular_dependency_error(self, tmp_path):
        """Test that circular dependencies cause error exit."""
        prd = {
            "metadata": {},
            "goal": "Test",
            "stories": [
                {"id": "s1", "title": "S1", "dependencies": ["s2"], "status": "pending"},
                {"id": "s2", "title": "S2", "dependencies": ["s1"], "status": "pending"},
            ]
        }

        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(prd, f)

        result = runner.invoke(
            test_app,
            ["--project", str(tmp_path), "--check"]
        )

        assert result.exit_code == 1
        assert "circular" in result.output.lower()

    def test_deps_help(self):
        """Test 'deps --help' shows options."""
        result = runner.invoke(test_app, ["--help"])

        assert result.exit_code == 0
        assert "--format" in result.output
        assert "--project" in result.output
        assert "--critical-path" in result.output
        assert "--check" in result.output


class TestDependencyGraphAnalyzerWithPathResolver:
    """Tests for DependencyGraphAnalyzer with PathResolver integration."""

    def test_analyzer_with_legacy_path_resolver(self, tmp_path, sample_prd):
        """Test analyzer using PathResolver in legacy mode (files in project root)."""
        # Create PRD in project root (legacy location)
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        # Create PathResolver in legacy mode
        path_resolver = PathResolver(tmp_path, legacy_mode=True)

        analyzer = DependencyGraphAnalyzer(tmp_path, path_resolver=path_resolver)
        result = analyzer.analyze()

        assert result.source_type == "prd"
        assert len(result.nodes) == 4
        assert "story-001" in result.nodes

    def test_analyzer_with_migrated_path_resolver(self, tmp_path, sample_prd):
        """Test analyzer using PathResolver in new mode (files in user data directory)."""
        # Create a separate data directory to simulate migration
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        # Create PathResolver with data_dir_override (simulating migrated project)
        path_resolver = PathResolver(
            tmp_path,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        # Create PRD in the path resolver's location (not project root)
        prd_path = path_resolver.get_prd_path()
        prd_path.parent.mkdir(parents=True, exist_ok=True)
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        analyzer = DependencyGraphAnalyzer(tmp_path, path_resolver=path_resolver)
        result = analyzer.analyze()

        assert result.source_type == "prd"
        assert len(result.nodes) == 4
        assert "story-001" in result.nodes

    def test_analyzer_with_mega_plan_migrated(self, tmp_path, sample_mega_plan):
        """Test analyzer using PathResolver in new mode for mega-plan files."""
        # Create a separate data directory to simulate migration
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        # Create PathResolver with data_dir_override
        path_resolver = PathResolver(
            tmp_path,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        # Create mega-plan in the path resolver's location
        mega_path = path_resolver.get_mega_plan_path()
        mega_path.parent.mkdir(parents=True, exist_ok=True)
        with open(mega_path, "w") as f:
            json.dump(sample_mega_plan, f)

        analyzer = DependencyGraphAnalyzer(tmp_path, path_resolver=path_resolver)
        result = analyzer.analyze()

        assert result.source_type == "mega-plan"
        assert len(result.nodes) == 3
        assert "feature-001" in result.nodes

    def test_analyzer_without_path_resolver_uses_legacy(self, tmp_path, sample_prd):
        """Test that analyzer without PathResolver uses legacy behavior."""
        # Create PRD in project root
        prd_path = tmp_path / "prd.json"
        with open(prd_path, "w") as f:
            json.dump(sample_prd, f)

        # Create analyzer without PathResolver (legacy behavior)
        analyzer = DependencyGraphAnalyzer(tmp_path)

        assert analyzer.prd_path == tmp_path / "prd.json"
        assert analyzer.mega_plan_path == tmp_path / "mega-plan.json"

        result = analyzer.analyze()
        assert result.source_type == "prd"

    def test_analyzer_file_not_found_in_migrated_location(self, tmp_path):
        """Test error when files don't exist in migrated location."""
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        path_resolver = PathResolver(
            tmp_path,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        # Don't create any files
        analyzer = DependencyGraphAnalyzer(tmp_path, path_resolver=path_resolver)

        with pytest.raises(FileNotFoundError):
            analyzer.analyze()

    def test_analyzer_path_resolver_paths_are_correct(self, tmp_path):
        """Test that analyzer uses PathResolver paths correctly."""
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        path_resolver = PathResolver(
            tmp_path,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        analyzer = DependencyGraphAnalyzer(tmp_path, path_resolver=path_resolver)

        # Verify paths are from PathResolver, not project root
        assert analyzer.prd_path == path_resolver.get_prd_path()
        assert analyzer.mega_plan_path == path_resolver.get_mega_plan_path()
        assert analyzer.prd_path != tmp_path / "prd.json"
