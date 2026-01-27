#!/usr/bin/env python3
"""
Merge Coordinator for Mega Plan

Coordinates the final merge of all features when complete.
Handles dependency-ordered merging and cleanup.
"""

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from .mega_state import MegaStateManager
from .mega_generator import MegaPlanGenerator


class MergeCoordinator:
    """Coordinates merging of all completed features."""

    def __init__(self, project_root: Path):
        """
        Initialize the merge coordinator.

        Args:
            project_root: Root directory of the project
        """
        self.project_root = Path(project_root)
        self.state_manager = MegaStateManager(project_root)
        self.mega_generator = MegaPlanGenerator(project_root)
        self.worktree_dir = self.project_root / ".worktree"

    def verify_all_features_complete(self) -> Tuple[bool, List[str]]:
        """
        Verify all features are complete.

        Returns:
            Tuple of (all_complete, list_of_incomplete_feature_ids)
        """
        plan = self.state_manager.read_mega_plan()
        if not plan:
            return False, ["No mega-plan found"]

        incomplete = []
        for feature in plan.get("features", []):
            if feature.get("status") != "complete":
                incomplete.append(feature["id"])

        return (len(incomplete) == 0, incomplete)

    def generate_merge_plan(self) -> List[Dict]:
        """
        Generate merge plan respecting dependency order.

        Features are merged in dependency order:
        - Features with no dependencies first
        - Then features whose dependencies are all merged

        Returns:
            Ordered list of features to merge
        """
        plan = self.state_manager.read_mega_plan()
        if not plan:
            return []

        # Use batch order - batches are already dependency-sorted
        batches = self.mega_generator.generate_feature_batches(plan)

        merge_order = []
        for batch in batches:
            for feature in batch:
                merge_order.append(feature)

        return merge_order

    def merge_feature(self, feature: Dict, target_branch: str) -> Tuple[bool, str]:
        """
        Merge a single feature into the target branch.

        Args:
            feature: Feature dictionary
            target_branch: Branch to merge into

        Returns:
            Tuple of (success, message)
        """
        feature_name = feature["name"]
        branch_name = f"mega-{feature_name}"
        worktree_path = self.worktree_dir / feature_name

        try:
            # First, checkout target branch
            subprocess.run(
                ["git", "checkout", target_branch],
                cwd=self.project_root,
                check=True,
                capture_output=True,
                text=True
            )

            # Merge the feature branch
            result = subprocess.run(
                ["git", "merge", branch_name, "--no-ff", "-m", f"Merge {feature['title']} ({feature['id']})"],
                cwd=self.project_root,
                capture_output=True,
                text=True
            )

            if result.returncode != 0:
                return False, f"Merge conflict or error: {result.stderr}"

            return True, f"Successfully merged {feature_name}"

        except subprocess.CalledProcessError as e:
            return False, f"Git error: {e.stderr}"
        except Exception as e:
            return False, f"Error: {str(e)}"

    def merge_all_features(self, target_branch: Optional[str] = None) -> Dict[str, Tuple[bool, str]]:
        """
        Merge all features in dependency order.

        Args:
            target_branch: Target branch (uses plan's target_branch if not specified)

        Returns:
            Dictionary mapping feature IDs to (success, message) tuples
        """
        plan = self.state_manager.read_mega_plan()
        if not plan:
            return {"error": (False, "No mega-plan found")}

        if target_branch is None:
            target_branch = plan.get("target_branch", "main")

        results = {}
        merge_order = self.generate_merge_plan()

        for feature in merge_order:
            success, message = self.merge_feature(feature, target_branch)
            results[feature["id"]] = (success, message)

            if not success:
                print(f"Merge failed for {feature['id']}: {message}")
                # Don't continue if a merge fails
                break
            else:
                print(f"Merged: {feature['id']} - {feature['title']}")

        return results

    def remove_worktree(self, feature_name: str) -> Tuple[bool, str]:
        """
        Remove a feature's worktree.

        Args:
            feature_name: Name of the feature

        Returns:
            Tuple of (success, message)
        """
        worktree_path = self.worktree_dir / feature_name

        if not worktree_path.exists():
            return True, f"Worktree already removed: {feature_name}"

        try:
            # Remove worktree using git
            subprocess.run(
                ["git", "worktree", "remove", str(worktree_path), "--force"],
                cwd=self.project_root,
                check=True,
                capture_output=True,
                text=True
            )
            return True, f"Removed worktree: {feature_name}"

        except subprocess.CalledProcessError as e:
            # Try manual removal if git command fails
            try:
                shutil.rmtree(worktree_path)
                # Also prune worktree
                subprocess.run(
                    ["git", "worktree", "prune"],
                    cwd=self.project_root,
                    capture_output=True,
                    text=True
                )
                return True, f"Manually removed worktree: {feature_name}"
            except Exception as ex:
                return False, f"Failed to remove worktree: {ex}"

    def delete_feature_branch(self, feature_name: str) -> Tuple[bool, str]:
        """
        Delete a feature's branch.

        Args:
            feature_name: Name of the feature

        Returns:
            Tuple of (success, message)
        """
        branch_name = f"mega-{feature_name}"

        try:
            subprocess.run(
                ["git", "branch", "-d", branch_name],
                cwd=self.project_root,
                check=True,
                capture_output=True,
                text=True
            )
            return True, f"Deleted branch: {branch_name}"

        except subprocess.CalledProcessError as e:
            # Try force delete if regular delete fails
            try:
                subprocess.run(
                    ["git", "branch", "-D", branch_name],
                    cwd=self.project_root,
                    check=True,
                    capture_output=True,
                    text=True
                )
                return True, f"Force deleted branch: {branch_name}"
            except subprocess.CalledProcessError:
                return False, f"Failed to delete branch: {e.stderr}"

    def cleanup_worktrees(self) -> Dict[str, Tuple[bool, str]]:
        """
        Clean up all feature worktrees.

        Returns:
            Dictionary mapping feature names to (success, message) tuples
        """
        plan = self.state_manager.read_mega_plan()
        if not plan:
            return {}

        results = {}

        for feature in plan.get("features", []):
            name = feature["name"]

            # Remove worktree
            success, message = self.remove_worktree(name)
            results[f"{name}-worktree"] = (success, message)
            if not success:
                print(f"Warning: {message}")

            # Delete branch
            success, message = self.delete_feature_branch(name)
            results[f"{name}-branch"] = (success, message)
            if not success:
                print(f"Warning: {message}")

        # Clean up worktree directory if empty
        try:
            if self.worktree_dir.exists():
                remaining = list(self.worktree_dir.iterdir())
                if not remaining:
                    self.worktree_dir.rmdir()
                    results["worktree-dir"] = (True, "Removed empty .worktree directory")
        except Exception as e:
            results["worktree-dir"] = (False, f"Could not remove .worktree: {e}")

        return results

    def cleanup_mega_files(self) -> Dict[str, Tuple[bool, str]]:
        """
        Clean up mega-plan related files.

        Returns:
            Dictionary mapping file names to (success, message) tuples
        """
        self.state_manager.cleanup_all()
        return {
            "mega-plan.json": (True, "Removed"),
            ".mega-status.json": (True, "Removed"),
            "mega-findings.md": (True, "Removed")
        }

    def complete_mega_plan(self, target_branch: Optional[str] = None) -> Dict:
        """
        Complete the mega-plan: merge all features, cleanup.

        Args:
            target_branch: Target branch for merging

        Returns:
            Summary dictionary with results
        """
        summary = {
            "success": True,
            "verification": {},
            "merge_results": {},
            "cleanup_results": {},
            "errors": []
        }

        # Step 1: Verify all complete
        all_complete, incomplete = self.verify_all_features_complete()
        summary["verification"]["all_complete"] = all_complete
        summary["verification"]["incomplete"] = incomplete

        if not all_complete:
            summary["success"] = False
            summary["errors"].append(f"Incomplete features: {', '.join(incomplete)}")
            return summary

        # Step 2: Merge all features
        merge_results = self.merge_all_features(target_branch)
        summary["merge_results"] = {k: {"success": v[0], "message": v[1]} for k, v in merge_results.items()}

        # Check for merge failures
        for fid, (success, _) in merge_results.items():
            if not success and fid != "error":
                summary["success"] = False
                summary["errors"].append(f"Merge failed for {fid}")
                return summary

        # Step 3: Cleanup worktrees and branches
        cleanup_results = self.cleanup_worktrees()
        summary["cleanup_results"]["worktrees"] = {k: {"success": v[0], "message": v[1]} for k, v in cleanup_results.items()}

        # Step 4: Cleanup mega files
        file_cleanup = self.cleanup_mega_files()
        summary["cleanup_results"]["files"] = {k: {"success": v[0], "message": v[1]} for k, v in file_cleanup.items()}

        return summary

    def generate_completion_summary(self, results: Dict) -> str:
        """
        Generate a human-readable completion summary.

        Args:
            results: Results from complete_mega_plan

        Returns:
            Formatted summary string
        """
        lines = [
            "=" * 60,
            "MEGA PLAN COMPLETION SUMMARY",
            "=" * 60,
            ""
        ]

        if results["success"]:
            lines.append("Status: SUCCESS")
        else:
            lines.append("Status: FAILED")
            if results.get("errors"):
                lines.append("\nErrors:")
                for error in results["errors"]:
                    lines.append(f"  - {error}")

        lines.append("")

        # Verification
        if results.get("verification"):
            v = results["verification"]
            lines.append("Verification:")
            lines.append(f"  All Complete: {v.get('all_complete', False)}")
            if v.get("incomplete"):
                lines.append(f"  Incomplete: {', '.join(v['incomplete'])}")
        lines.append("")

        # Merge Results
        if results.get("merge_results"):
            lines.append("Merge Results:")
            for fid, result in results["merge_results"].items():
                status = "OK" if result["success"] else "FAILED"
                lines.append(f"  [{status}] {fid}: {result['message']}")
        lines.append("")

        # Cleanup Results
        if results.get("cleanup_results"):
            cr = results["cleanup_results"]
            lines.append("Cleanup:")
            if cr.get("worktrees"):
                for name, result in cr["worktrees"].items():
                    status = "OK" if result["success"] else "WARN"
                    lines.append(f"  [{status}] {name}")
            if cr.get("files"):
                for name, result in cr["files"].items():
                    status = "OK" if result["success"] else "WARN"
                    lines.append(f"  [{status}] {name}")

        lines.append("")
        lines.append("=" * 60)

        return "\n".join(lines)


def main():
    """CLI interface for testing merge coordinator."""
    import sys

    if len(sys.argv) < 2:
        print("Usage: merge_coordinator.py <command> [args]")
        print("Commands:")
        print("  verify              - Verify all features complete")
        print("  plan                - Show merge plan")
        print("  complete [branch]   - Complete mega-plan")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    mc = MergeCoordinator(project_root)

    if command == "verify":
        all_complete, incomplete = mc.verify_all_features_complete()
        if all_complete:
            print("All features are complete!")
        else:
            print("Incomplete features:")
            for fid in incomplete:
                print(f"  - {fid}")

    elif command == "plan":
        merge_order = mc.generate_merge_plan()
        if not merge_order:
            print("No features to merge")
        else:
            print("Merge Order:")
            for i, feature in enumerate(merge_order, 1):
                print(f"  {i}. {feature['id']}: {feature['title']}")

    elif command == "complete":
        target_branch = sys.argv[2] if len(sys.argv) > 2 else None
        results = mc.complete_mega_plan(target_branch)
        print(mc.generate_completion_summary(results))

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
