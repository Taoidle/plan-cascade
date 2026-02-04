#!/usr/bin/env python3
"""
Feature Orchestrator for Mega Plan

Manages the execution of features as hybrid:worktree tasks.
Handles worktree creation, PRD generation, and batch execution.
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional, Tuple

try:
    from .mega_state import MegaStateManager
    from .mega_generator import MegaPlanGenerator
except ImportError:  # Allow direct script execution
    from mega_state import MegaStateManager
    from mega_generator import MegaPlanGenerator


class FeatureOrchestrator:
    """Orchestrates feature execution using hybrid:worktree."""

    def __init__(self, project_root: Path):
        """
        Initialize the feature orchestrator.

        Args:
            project_root: Root directory of the project
        """
        self.project_root = Path(project_root)
        self.state_manager = MegaStateManager(project_root)
        self.mega_generator = MegaPlanGenerator(project_root)
        self.worktree_dir = self.project_root / ".worktree"

    def create_feature_worktrees(self, batch: List[Dict], target_branch: str) -> List[Tuple[str, Path]]:
        """
        Create worktrees for a batch of features.

        Args:
            batch: List of feature dictionaries
            target_branch: Target branch for merging

        Returns:
            List of (feature_id, worktree_path) tuples
        """
        results = []
        self.worktree_dir.mkdir(exist_ok=True)

        for feature in batch:
            feature_name = feature["name"]
            worktree_path = self.worktree_dir / feature_name

            if worktree_path.exists():
                print(f"Worktree already exists: {worktree_path}")
                results.append((feature["id"], worktree_path))
                continue

            # Create branch name
            branch_name = f"mega-{feature_name}"

            try:
                # Create worktree
                subprocess.run(
                    ["git", "worktree", "add", "-b", branch_name, str(worktree_path)],
                    cwd=self.project_root,
                    check=True,
                    capture_output=True,
                    text=True
                )
                print(f"Created worktree: {worktree_path}")
                results.append((feature["id"], worktree_path))

                # Update feature status
                self.state_manager.update_feature_status(feature["id"], "prd_generated")

                # Copy mega-findings to worktree
                self.state_manager.copy_mega_findings_to_worktree(feature_name)

            except subprocess.CalledProcessError as e:
                print(f"Error creating worktree for {feature_name}: {e.stderr}")
                self.state_manager.update_feature_status(feature["id"], "failed")

        return results

    def generate_feature_prd(self, feature: Dict, worktree_path: Path) -> bool:
        """
        Generate a PRD for a feature in its worktree.

        Args:
            feature: Feature dictionary
            worktree_path: Path to the feature worktree

        Returns:
            True if PRD generated successfully
        """
        prd_path = worktree_path / "prd.json"

        # Create basic PRD structure that can be enhanced by LLM
        prd = {
            "metadata": {
                "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
                "version": "1.0.0",
                "description": feature["description"],
                "mega_feature_id": feature["id"],
                "mega_feature_name": feature["name"]
            },
            "goal": feature["title"],
            "objectives": [],
            "stories": []
        }

        try:
            with open(prd_path, "w", encoding="utf-8") as f:
                json.dump(prd, f, indent=2)

            # Create findings.md
            findings_path = worktree_path / "findings.md"
            if not findings_path.exists():
                with open(findings_path, "w", encoding="utf-8") as f:
                    f.write(f"# Findings: {feature['title']}\n\n")
                    f.write(f"Feature: {feature['name']} ({feature['id']})\n\n")
                    f.write("---\n\n")

            # Create progress.txt
            progress_path = worktree_path / "progress.txt"
            if not progress_path.exists():
                with open(progress_path, "w", encoding="utf-8") as f:
                    timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
                    f.write(f"[{timestamp}] Feature {feature['id']} initialized\n")

            # Create .planning-config.json
            config_path = worktree_path / ".planning-config.json"
            config = {
                "task_name": feature["name"],
                "target_branch": self.state_manager.read_mega_plan().get("target_branch", "main"),
                "mega_feature_id": feature["id"],
                "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ")
            }
            with open(config_path, "w", encoding="utf-8") as f:
                json.dump(config, f, indent=2)

            return True

        except Exception as e:
            print(f"Error generating PRD for {feature['name']}: {e}")
            return False

    def generate_feature_prds(self, batch: List[Dict]) -> Dict[str, bool]:
        """
        Generate PRDs for all features in a batch.

        Args:
            batch: List of feature dictionaries

        Returns:
            Dictionary mapping feature IDs to success status
        """
        results = {}

        for feature in batch:
            worktree_path = self.state_manager.get_worktree_path(feature["name"])
            if worktree_path.exists():
                success = self.generate_feature_prd(feature, worktree_path)
                results[feature["id"]] = success
            else:
                results[feature["id"]] = False

        return results

    def check_prd_approval(self, feature: Dict) -> bool:
        """
        Check if a feature's PRD has been approved.

        A PRD is considered approved if:
        1. The prd.json exists and has stories
        2. The feature status is 'approved' or beyond

        Args:
            feature: Feature dictionary

        Returns:
            True if PRD is approved
        """
        worktree_path = self.state_manager.get_worktree_path(feature["name"])
        prd_path = worktree_path / "prd.json"

        if not prd_path.exists():
            return False

        try:
            with open(prd_path, "r", encoding="utf-8") as f:
                prd = json.load(f)

            # Check if stories exist
            stories = prd.get("stories", [])
            if not stories:
                return False

            # Check feature status
            plan = self.state_manager.read_mega_plan()
            for f in plan.get("features", []):
                if f["id"] == feature["id"]:
                    return f["status"] in ["approved", "in_progress", "complete"]

            return False

        except Exception:
            return False

    def wait_for_prd_approvals(self, batch: List[Dict], timeout: int = 3600) -> Dict[str, bool]:
        """
        Wait for PRD approvals for all features in a batch.

        Args:
            batch: List of feature dictionaries
            timeout: Maximum time to wait in seconds

        Returns:
            Dictionary mapping feature IDs to approval status
        """
        start_time = time.time()
        results = {f["id"]: False for f in batch}

        while time.time() - start_time < timeout:
            all_approved = True

            for feature in batch:
                if results[feature["id"]]:
                    continue

                if self.check_prd_approval(feature):
                    results[feature["id"]] = True
                    print(f"PRD approved: {feature['name']}")
                else:
                    all_approved = False

            if all_approved:
                break

            time.sleep(5)  # Check every 5 seconds

        return results

    def auto_approve_prd(self, feature: Dict) -> bool:
        """
        Automatically approve a feature's PRD.

        Args:
            feature: Feature dictionary

        Returns:
            True if successfully approved
        """
        worktree_path = self.state_manager.get_worktree_path(feature["name"])
        prd_path = worktree_path / "prd.json"

        if not prd_path.exists():
            return False

        try:
            # Update feature status to approved
            self.state_manager.update_feature_status(feature["id"], "approved")
            return True
        except Exception as e:
            print(f"Error auto-approving PRD for {feature['name']}: {e}")
            return False

    def execute_feature_batch(self, batch: List[Dict], auto_prd: bool = False) -> Dict[str, str]:
        """
        Execute a batch of features.

        Args:
            batch: List of feature dictionaries
            auto_prd: If True, automatically approve generated PRDs

        Returns:
            Dictionary mapping feature IDs to execution status
        """
        results = {}

        for feature in batch:
            # Update status to in_progress
            self.state_manager.update_feature_status(feature["id"], "in_progress")

            if auto_prd:
                # Auto-approve the PRD
                self.auto_approve_prd(feature)

            # The actual story execution happens through hybrid-ralph
            # This orchestrator just manages the feature-level coordination
            results[feature["id"]] = "in_progress"

        return results

    def check_feature_complete(self, feature: Dict) -> bool:
        """
        Check if a feature is complete.

        A feature is complete when all its stories are complete.

        Args:
            feature: Feature dictionary

        Returns:
            True if feature is complete
        """
        worktree_path = self.state_manager.get_worktree_path(feature["name"])
        prd_path = worktree_path / "prd.json"

        if not prd_path.exists():
            return False

        try:
            with open(prd_path, "r", encoding="utf-8") as f:
                prd = json.load(f)

            stories = prd.get("stories", [])
            if not stories:
                return False

            for story in stories:
                if story.get("status") != "complete":
                    return False

            return True

        except Exception:
            return False

    def check_batch_complete(self, batch: List[Dict]) -> bool:
        """
        Check if all features in a batch are complete.

        Args:
            batch: List of feature dictionaries

        Returns:
            True if all features are complete
        """
        for feature in batch:
            if not self.check_feature_complete(feature):
                return False
        return True

    def get_batch_status(self, batch: List[Dict]) -> Dict[str, Dict]:
        """
        Get detailed status for all features in a batch.

        Args:
            batch: List of feature dictionaries

        Returns:
            Dictionary mapping feature IDs to status details
        """
        results = {}

        for feature in batch:
            worktree_path = self.state_manager.get_worktree_path(feature["name"])
            prd_path = worktree_path / "prd.json"

            status = {
                "name": feature["name"],
                "title": feature["title"],
                "worktree_exists": worktree_path.exists(),
                "prd_exists": prd_path.exists(),
                "stories_total": 0,
                "stories_complete": 0,
                "feature_status": feature.get("status", "pending")
            }

            if prd_path.exists():
                try:
                    with open(prd_path, "r", encoding="utf-8") as f:
                        prd = json.load(f)
                    stories = prd.get("stories", [])
                    status["stories_total"] = len(stories)
                    status["stories_complete"] = sum(1 for s in stories if s.get("status") == "complete")
                except Exception:
                    pass

            results[feature["id"]] = status

        return results

    def generate_execution_plan(self) -> str:
        """
        Generate a human-readable execution plan.

        Returns:
            Execution plan as formatted string
        """
        plan = self.state_manager.read_mega_plan()
        if not plan:
            return "No mega-plan found"

        batches = self.mega_generator.generate_feature_batches(plan)

        lines = [
            "=" * 60,
            "MEGA PLAN EXECUTION PLAN",
            "=" * 60,
            "",
            f"Goal: {plan.get('goal', 'N/A')}",
            f"Execution Mode: {plan.get('execution_mode', 'auto')}",
            f"Target Branch: {plan.get('target_branch', 'main')}",
            f"Total Features: {len(plan.get('features', []))}",
            f"Total Batches: {len(batches)}",
            ""
        ]

        for i, batch in enumerate(batches, 1):
            lines.append(f"Batch {i}:")
            for feature in batch:
                deps = feature.get("dependencies", [])
                dep_str = f" (depends on: {', '.join(deps)})" if deps else ""
                status = feature.get("status", "pending")
                lines.append(f"  [{status}] {feature['id']}: {feature['title']}{dep_str}")
            lines.append("")

        lines.append("=" * 60)
        return "\n".join(lines)

    def print_status(self) -> None:
        """Print current execution status."""
        plan = self.state_manager.read_mega_plan()
        if not plan:
            print("No mega-plan found")
            return

        progress = self.mega_generator.calculate_progress(plan)
        batches = self.mega_generator.generate_feature_batches(plan)

        print("\n" + "=" * 60)
        print("MEGA PLAN STATUS")
        print("=" * 60)
        print(f"\nProgress: {progress['percentage']}%")
        print(f"  Completed: {progress['completed']}/{progress['total']}")
        print(f"  In Progress: {progress['in_progress']}")
        print(f"  Pending: {progress['pending']}")
        if progress['failed'] > 0:
            print(f"  Failed: {progress['failed']}")

        print("\nFeatures:")
        for i, batch in enumerate(batches, 1):
            print(f"\n  Batch {i}:")
            for feature in batch:
                status = feature.get("status", "pending")
                symbol = {
                    "pending": " ",
                    "prd_generated": "~",
                    "approved": "~",
                    "in_progress": ">",
                    "complete": "X",
                    "failed": "!"
                }.get(status, "?")

                # Get story progress if available
                worktree_path = self.state_manager.get_worktree_path(feature["name"])
                story_info = ""
                if worktree_path.exists():
                    prd_path = worktree_path / "prd.json"
                    if prd_path.exists():
                        try:
                            with open(prd_path, "r", encoding="utf-8") as f:
                                prd = json.load(f)
                            stories = prd.get("stories", [])
                            if stories:
                                complete = sum(1 for s in stories if s.get("status") == "complete")
                                story_info = f" [{complete}/{len(stories)} stories]"
                        except Exception:
                            pass

                print(f"    [{symbol}] {feature['id']}: {feature['title']}{story_info}")

        print("\n" + "=" * 60)


def main():
    """CLI interface for testing feature orchestrator."""
    import sys

    if len(sys.argv) < 2:
        print("Usage: feature_orchestrator.py <command> [args]")
        print("Commands:")
        print("  plan                  - Show execution plan")
        print("  status                - Show execution status")
        print("  batch-status <n>      - Show status for batch N")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    fo = FeatureOrchestrator(project_root)

    if command == "plan":
        print(fo.generate_execution_plan())

    elif command == "status":
        fo.print_status()

    elif command == "batch-status" and len(sys.argv) >= 3:
        batch_num = int(sys.argv[2])
        plan = fo.state_manager.read_mega_plan()
        if not plan:
            print("No mega-plan found")
            sys.exit(1)

        batches = fo.mega_generator.generate_feature_batches(plan)
        if 1 <= batch_num <= len(batches):
            batch = batches[batch_num - 1]
            status = fo.get_batch_status(batch)
            print(json.dumps(status, indent=2))
        else:
            print(f"Invalid batch number. Must be 1-{len(batches)}")

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
