//! Dependency Analyzer
//!
//! Analyzes PRD story dependencies and generates execution batches.
//! Detects circular dependencies and provides visual graph output.

use std::collections::{HashMap, HashSet};

use crate::models::prd::{Prd, Story, StoryStatus};

/// A batch of stories that can be executed in parallel
#[derive(Debug, Clone)]
pub struct Batch {
    /// Batch index (1-based for display)
    pub index: usize,
    /// Story IDs in this batch
    pub story_ids: Vec<String>,
}

impl Batch {
    /// Create a new batch
    pub fn new(index: usize, story_ids: Vec<String>) -> Self {
        Self { index, story_ids }
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.story_ids.is_empty()
    }

    /// Get number of stories in batch
    pub fn len(&self) -> usize {
        self.story_ids.len()
    }
}

/// Errors that can occur during dependency analysis
#[derive(Debug, thiserror::Error)]
pub enum DependencyError {
    #[error("Circular dependency detected: {0:?}")]
    CircularDependency(Vec<String>),

    #[error("Unknown dependency '{dependency}' in story '{story}'")]
    UnknownDependency { story: String, dependency: String },

    #[error("Empty PRD")]
    EmptyPrd,
}

/// Dependency analyzer for PRD stories
pub struct DependencyAnalyzer;

impl DependencyAnalyzer {
    /// Generate execution batches from PRD stories
    ///
    /// Stories are organized into batches where each batch contains stories
    /// whose dependencies are all satisfied by previous batches.
    ///
    /// # Arguments
    /// * `prd` - The PRD containing stories to analyze
    ///
    /// # Returns
    /// * `Ok(Vec<Batch>)` - Ordered batches of story IDs
    /// * `Err(DependencyError)` - If circular dependencies are detected
    pub fn generate_batches(prd: &Prd) -> Result<Vec<Batch>, DependencyError> {
        if prd.stories.is_empty() {
            return Ok(Vec::new());
        }

        // Validate all dependencies exist
        Self::validate_dependencies(prd)?;

        let mut batches = Vec::new();
        let mut completed: HashSet<String> = HashSet::new();
        let mut remaining: HashSet<String> = prd
            .stories
            .iter()
            .filter(|s| s.status != StoryStatus::Completed)
            .map(|s| s.id.clone())
            .collect();

        // Include already completed stories in the completed set
        for story in &prd.stories {
            if story.status == StoryStatus::Completed {
                completed.insert(story.id.clone());
            }
        }

        while !remaining.is_empty() {
            let mut batch = Vec::new();

            for story in &prd.stories {
                if remaining.contains(&story.id) {
                    // Check if all dependencies are satisfied
                    let deps_satisfied =
                        story.dependencies.iter().all(|dep| completed.contains(dep));

                    if deps_satisfied {
                        batch.push(story.id.clone());
                    }
                }
            }

            if batch.is_empty() {
                // Circular dependency detected
                let cycle = Self::find_cycle(&prd.stories, &remaining);
                return Err(DependencyError::CircularDependency(cycle));
            }

            // Move batch items to completed
            for id in &batch {
                remaining.remove(id);
                completed.insert(id.clone());
            }

            batches.push(Batch::new(batches.len() + 1, batch));
        }

        Ok(batches)
    }

    /// Validate that all dependencies reference existing stories
    fn validate_dependencies(prd: &Prd) -> Result<(), DependencyError> {
        let story_ids: HashSet<_> = prd.stories.iter().map(|s| s.id.as_str()).collect();

        for story in &prd.stories {
            for dep in &story.dependencies {
                if !story_ids.contains(dep.as_str()) {
                    return Err(DependencyError::UnknownDependency {
                        story: story.id.clone(),
                        dependency: dep.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Detect circular dependencies using DFS
    fn find_cycle(stories: &[Story], remaining: &HashSet<String>) -> Vec<String> {
        let story_map: HashMap<_, _> = stories.iter().map(|s| (s.id.as_str(), s)).collect();

        for start_id in remaining {
            let mut visited = HashSet::new();
            let mut path = Vec::new();

            if Self::dfs_find_cycle(start_id, &story_map, &mut visited, &mut path) {
                // Trim path to just the cycle
                if let Some(pos) = path.iter().position(|id| id == path.last().unwrap()) {
                    return path[pos..].to_vec();
                }
                return path;
            }
        }

        Vec::new()
    }

    /// DFS helper to find cycles
    fn dfs_find_cycle(
        current: &str,
        story_map: &HashMap<&str, &Story>,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> bool {
        // Check if current is already in path (cycle detected)
        if path.contains(&current.to_string()) {
            path.push(current.to_string());
            return true;
        }

        // Skip if already fully visited
        if visited.contains(current) {
            return false;
        }

        path.push(current.to_string());

        if let Some(story) = story_map.get(current) {
            for dep in &story.dependencies {
                if Self::dfs_find_cycle(dep, story_map, visited, path) {
                    return true;
                }
            }
        }

        path.pop();
        visited.insert(current.to_string());
        false
    }

    /// Generate a visual ASCII dependency graph
    pub fn generate_graph_ascii(prd: &Prd) -> String {
        let mut output = String::new();
        output.push_str("Dependency Graph\n");
        output.push_str("================\n\n");

        if prd.stories.is_empty() {
            output.push_str("(No stories)\n");
            return output;
        }

        // Generate batches for ordering
        let batches = match Self::generate_batches(prd) {
            Ok(b) => b,
            Err(e) => {
                output.push_str(&format!("Error: {}\n", e));
                return output;
            }
        };

        // Build reverse dependency map (what depends on each story)
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
        for story in &prd.stories {
            for dep in &story.dependencies {
                dependents.entry(dep.as_str()).or_default().push(&story.id);
            }
        }

        // Print batch information
        for batch in &batches {
            output.push_str(&format!("Batch {}: ", batch.index));
            output.push_str(&batch.story_ids.join(", "));
            output.push('\n');
        }
        output.push('\n');

        // Print each story with its dependencies
        output.push_str("Stories:\n");
        output.push_str("--------\n");

        for story in &prd.stories {
            let status_icon = match story.status {
                StoryStatus::Completed => "[x]",
                StoryStatus::InProgress => "[~]",
                StoryStatus::Failed => "[!]",
                StoryStatus::Skipped => "[-]",
                StoryStatus::Pending => "[ ]",
            };

            output.push_str(&format!("{} {} - {}\n", status_icon, story.id, story.title));

            if !story.dependencies.is_empty() {
                output.push_str(&format!(
                    "    <- depends on: {}\n",
                    story.dependencies.join(", ")
                ));
            }

            if let Some(deps) = dependents.get(story.id.as_str()) {
                output.push_str(&format!("    -> required by: {}\n", deps.join(", ")));
            }
        }

        // Print ASCII tree representation
        output.push_str("\nTree View:\n");
        output.push_str("----------\n");

        let roots: Vec<_> = prd
            .stories
            .iter()
            .filter(|s| s.dependencies.is_empty())
            .collect();

        for (i, root) in roots.iter().enumerate() {
            let is_last = i == roots.len() - 1;
            Self::print_tree_node(&mut output, root, &prd.stories, &dependents, "", is_last);
        }

        output
    }

    /// Recursively print a tree node
    fn print_tree_node(
        output: &mut String,
        story: &Story,
        all_stories: &[Story],
        dependents: &HashMap<&str, Vec<&str>>,
        prefix: &str,
        is_last: bool,
    ) {
        let connector = if is_last { "\\-- " } else { "|-- " };
        let status = match story.status {
            StoryStatus::Completed => "[x]",
            StoryStatus::InProgress => "[~]",
            StoryStatus::Failed => "[!]",
            _ => "[ ]",
        };

        output.push_str(&format!("{}{}{} {}\n", prefix, connector, status, story.id));

        // Get children (stories that depend on this one)
        if let Some(children) = dependents.get(story.id.as_str()) {
            let new_prefix = format!("{}{}   ", prefix, if is_last { " " } else { "|" });

            for (i, child_id) in children.iter().enumerate() {
                if let Some(child) = all_stories.iter().find(|s| s.id == *child_id) {
                    let child_is_last = i == children.len() - 1;
                    Self::print_tree_node(
                        output,
                        child,
                        all_stories,
                        dependents,
                        &new_prefix,
                        child_is_last,
                    );
                }
            }
        }
    }

    /// Get the critical path (longest dependency chain)
    pub fn get_critical_path(prd: &Prd) -> Vec<String> {
        let story_map: HashMap<_, _> = prd.stories.iter().map(|s| (s.id.as_str(), s)).collect();

        let mut longest_path = Vec::new();

        // Start DFS from each story
        for story in &prd.stories {
            let mut visited = HashSet::new();
            let path = Self::dfs_longest_path(&story.id, &story_map, &mut visited);
            if path.len() > longest_path.len() {
                longest_path = path;
            }
        }

        longest_path
    }

    /// Find longest path using DFS
    fn dfs_longest_path(
        current: &str,
        story_map: &HashMap<&str, &Story>,
        visited: &mut HashSet<String>,
    ) -> Vec<String> {
        if visited.contains(current) {
            return Vec::new();
        }

        visited.insert(current.to_string());

        let mut longest = vec![current.to_string()];

        if let Some(story) = story_map.get(current) {
            for dep in &story.dependencies {
                let path = Self::dfs_longest_path(dep, story_map, visited);
                if path.len() + 1 > longest.len() {
                    longest = vec![current.to_string()];
                    longest.extend(path);
                }
            }
        }

        visited.remove(current);
        longest
    }

    /// Calculate metrics for the dependency graph
    pub fn calculate_metrics(prd: &Prd) -> DependencyMetrics {
        let total_stories = prd.stories.len();
        let total_dependencies: usize = prd.stories.iter().map(|s| s.dependencies.len()).sum();

        let batches = Self::generate_batches(prd).unwrap_or_default();
        let critical_path = Self::get_critical_path(prd);

        // Find bottlenecks (stories that many others depend on)
        let mut dependency_counts: HashMap<&str, usize> = HashMap::new();
        for story in &prd.stories {
            for dep in &story.dependencies {
                *dependency_counts.entry(dep.as_str()).or_insert(0) += 1;
            }
        }

        let bottlenecks: Vec<String> = dependency_counts
            .iter()
            .filter(|(_, count)| **count >= 2)
            .map(|(id, _)| id.to_string())
            .collect();

        DependencyMetrics {
            total_stories,
            total_dependencies,
            batch_count: batches.len(),
            max_parallel: batches.iter().map(|b| b.len()).max().unwrap_or(0),
            critical_path_length: critical_path.len(),
            critical_path,
            bottlenecks,
        }
    }
}

/// Metrics about the dependency graph
#[derive(Debug, Clone)]
pub struct DependencyMetrics {
    /// Total number of stories
    pub total_stories: usize,
    /// Total number of dependency edges
    pub total_dependencies: usize,
    /// Number of batches
    pub batch_count: usize,
    /// Maximum stories that can run in parallel
    pub max_parallel: usize,
    /// Length of the critical path
    pub critical_path_length: usize,
    /// Stories in the critical path
    pub critical_path: Vec<String>,
    /// Stories that are bottlenecks (many depend on them)
    pub bottlenecks: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_prd() -> Prd {
        let mut prd = Prd::new("Test PRD");

        // S001 -> no deps
        prd.add_story(Story::new("S001", "Setup project"));

        // S002 -> S001
        let mut s2 = Story::new("S002", "Add authentication");
        s2.dependencies = vec!["S001".to_string()];
        prd.add_story(s2);

        // S003 -> S001
        let mut s3 = Story::new("S003", "Add database");
        s3.dependencies = vec!["S001".to_string()];
        prd.add_story(s3);

        // S004 -> S002, S003
        let mut s4 = Story::new("S004", "Add user management");
        s4.dependencies = vec!["S002".to_string(), "S003".to_string()];
        prd.add_story(s4);

        prd
    }

    #[test]
    fn test_generate_batches() {
        let prd = create_test_prd();
        let batches = DependencyAnalyzer::generate_batches(&prd).unwrap();

        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].story_ids, vec!["S001"]);
        assert!(batches[1].story_ids.contains(&"S002".to_string()));
        assert!(batches[1].story_ids.contains(&"S003".to_string()));
        assert_eq!(batches[2].story_ids, vec!["S004"]);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut prd = Prd::new("Circular Test");

        let mut s1 = Story::new("S001", "Story 1");
        s1.dependencies = vec!["S002".to_string()];
        prd.add_story(s1);

        let mut s2 = Story::new("S002", "Story 2");
        s2.dependencies = vec!["S001".to_string()];
        prd.add_story(s2);

        let result = DependencyAnalyzer::generate_batches(&prd);
        assert!(matches!(
            result,
            Err(DependencyError::CircularDependency(_))
        ));
    }

    #[test]
    fn test_unknown_dependency() {
        let mut prd = Prd::new("Unknown Dep Test");

        let mut s1 = Story::new("S001", "Story 1");
        s1.dependencies = vec!["S999".to_string()];
        prd.add_story(s1);

        let result = DependencyAnalyzer::generate_batches(&prd);
        assert!(matches!(
            result,
            Err(DependencyError::UnknownDependency { .. })
        ));
    }

    #[test]
    fn test_empty_prd() {
        let prd = Prd::new("Empty");
        let batches = DependencyAnalyzer::generate_batches(&prd).unwrap();
        assert!(batches.is_empty());
    }

    #[test]
    fn test_generate_graph_ascii() {
        let prd = create_test_prd();
        let graph = DependencyAnalyzer::generate_graph_ascii(&prd);

        assert!(graph.contains("Dependency Graph"));
        assert!(graph.contains("Batch 1"));
        assert!(graph.contains("S001"));
    }

    #[test]
    fn test_critical_path() {
        let prd = create_test_prd();
        let path = DependencyAnalyzer::get_critical_path(&prd);

        // Critical path should include S004 at the start and S001 at the end
        assert!(!path.is_empty());
        assert!(path.contains(&"S001".to_string()));
    }

    #[test]
    fn test_metrics() {
        let prd = create_test_prd();
        let metrics = DependencyAnalyzer::calculate_metrics(&prd);

        assert_eq!(metrics.total_stories, 4);
        assert_eq!(metrics.total_dependencies, 4);
        assert_eq!(metrics.batch_count, 3);
        assert_eq!(metrics.max_parallel, 2); // S002 and S003 can run in parallel
    }

    #[test]
    fn test_completed_stories_excluded() {
        let mut prd = create_test_prd();
        prd.stories[0].status = StoryStatus::Completed; // S001 completed

        let batches = DependencyAnalyzer::generate_batches(&prd).unwrap();

        // S001 should not be in any batch since it's completed
        assert!(!batches[0].story_ids.contains(&"S001".to_string()));
        // S002 and S003 should now be in batch 1
        assert!(batches[0].story_ids.contains(&"S002".to_string()));
    }
}
