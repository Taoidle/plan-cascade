//! Commit DAG Layout Algorithm
//!
//! Computes a visual layout for commit history graphs.
//! Assigns each commit to a lane (column) and row,
//! then generates edges between parent-child pairs.
//!
//! Algorithm rules:
//! 1. Main branch stays on lane 0
//! 2. First-parent inherits lane from child
//! 3. Fork commits get minimum available lane
//! 4. Lanes are released when a branch merges back

use std::collections::{HashMap, HashSet};

use super::types::{CommitNode, GraphEdge, GraphLayout, GraphNode};

/// Compute graph layout from a topologically-ordered list of commits.
///
/// The input `commits` should be ordered newest-first (as `git log` outputs).
/// Returns a `GraphLayout` with positioned nodes and edges.
pub fn compute_graph_layout(commits: &[CommitNode]) -> GraphLayout {
    if commits.is_empty() {
        return GraphLayout::default();
    }

    // Build a SHA -> row index map
    let sha_to_row: HashMap<&str, u32> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| (c.sha.as_str(), i as u32))
        .collect();

    // Track which lanes are in use
    let mut active_lanes: HashSet<u32> = HashSet::new();
    // Map from SHA -> assigned lane
    let mut sha_to_lane: HashMap<&str, u32> = HashMap::new();
    // Map from lane -> SHA of the commit that "owns" this lane going down
    let mut lane_owner: HashMap<u32, &str> = HashMap::new();

    let mut nodes = Vec::with_capacity(commits.len());
    let mut edges = Vec::new();
    let mut max_lane: u32 = 0;

    for (row, commit) in commits.iter().enumerate() {
        let row = row as u32;

        // Determine lane for this commit
        let lane = if let Some(&l) = sha_to_lane.get(commit.sha.as_str()) {
            // Lane was pre-assigned by a child commit
            l
        } else {
            // No child assigned a lane — this is a branch tip
            // Find minimum available lane
            find_min_available_lane(&active_lanes)
        };

        // Activate lane
        active_lanes.insert(lane);
        sha_to_lane.insert(&commit.sha, lane);
        lane_owner.insert(lane, &commit.sha);

        if lane > max_lane {
            max_lane = lane;
        }

        nodes.push(GraphNode {
            sha: commit.sha.clone(),
            row,
            lane,
        });

        // Process parents
        for (i, parent_sha) in commit.parents.iter().enumerate() {
            // Only create edges for parents that exist in our commit set
            if let Some(&parent_row) = sha_to_row.get(parent_sha.as_str()) {
                let parent_lane = if i == 0 {
                    // First parent inherits this commit's lane
                    if !sha_to_lane.contains_key(parent_sha.as_str()) {
                        sha_to_lane.insert(parent_sha, lane);
                    }
                    *sha_to_lane.get(parent_sha.as_str()).unwrap_or(&lane)
                } else {
                    // Non-first parents (merge sources) get their own lane
                    if let Some(&existing) = sha_to_lane.get(parent_sha.as_str()) {
                        existing
                    } else {
                        let new_lane = find_min_available_lane(&active_lanes);
                        active_lanes.insert(new_lane);
                        sha_to_lane.insert(parent_sha, new_lane);
                        if new_lane > max_lane {
                            max_lane = new_lane;
                        }
                        new_lane
                    }
                };

                edges.push(GraphEdge {
                    from_sha: commit.sha.clone(),
                    to_sha: parent_sha.clone(),
                    from_lane: lane,
                    to_lane: parent_lane,
                    from_row: row,
                    to_row: parent_row,
                });
            }
        }

        // Release lanes: if this commit is a merge (>1 parents), release
        // lanes of non-first parents whose last child was this commit
        if commit.parents.len() > 1 {
            for parent_sha in commit.parents.iter().skip(1) {
                if let Some(&parent_lane) = sha_to_lane.get(parent_sha.as_str()) {
                    // Check if this lane is different from the main lane and
                    // no other commit needs it
                    if parent_lane != lane {
                        // The merge source lane can be released once we
                        // process the parent at that lane
                        // We mark it for potential release
                        if let Some(&owner) = lane_owner.get(&parent_lane) {
                            if owner == commit.sha.as_str() {
                                active_lanes.remove(&parent_lane);
                                lane_owner.remove(&parent_lane);
                            }
                        }
                    }
                }
            }
        }
    }

    GraphLayout {
        nodes,
        edges,
        max_lane,
    }
}

/// Find the minimum lane number not currently in use.
fn find_min_available_lane(active_lanes: &HashSet<u32>) -> u32 {
    let mut lane = 0;
    while active_lanes.contains(&lane) {
        lane += 1;
    }
    lane
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_commit(sha: &str, parents: Vec<&str>, refs: Vec<&str>) -> CommitNode {
        CommitNode {
            sha: sha.to_string(),
            short_sha: sha[..std::cmp::min(7, sha.len())].to_string(),
            parents: parents.into_iter().map(|s| s.to_string()).collect(),
            author_name: "Test".to_string(),
            author_email: "test@test.com".to_string(),
            date: "2026-02-19T10:00:00Z".to_string(),
            message: format!("commit {}", sha),
            refs: refs.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_empty_commits() {
        let layout = compute_graph_layout(&[]);
        assert!(layout.nodes.is_empty());
        assert!(layout.edges.is_empty());
        assert_eq!(layout.max_lane, 0);
    }

    #[test]
    fn test_single_commit() {
        let commits = vec![make_commit("aaa", vec![], vec!["HEAD -> main"])];
        let layout = compute_graph_layout(&commits);
        assert_eq!(layout.nodes.len(), 1);
        assert_eq!(layout.nodes[0].lane, 0);
        assert_eq!(layout.nodes[0].row, 0);
        assert!(layout.edges.is_empty());
        assert_eq!(layout.max_lane, 0);
    }

    #[test]
    fn test_linear_history() {
        // c3 -> c2 -> c1 (newest first)
        let commits = vec![
            make_commit("ccc", vec!["bbb"], vec!["HEAD -> main"]),
            make_commit("bbb", vec!["aaa"], vec![]),
            make_commit("aaa", vec![], vec![]),
        ];
        let layout = compute_graph_layout(&commits);
        assert_eq!(layout.nodes.len(), 3);

        // All on lane 0
        for node in &layout.nodes {
            assert_eq!(node.lane, 0);
        }

        // Two edges
        assert_eq!(layout.edges.len(), 2);
        assert_eq!(layout.max_lane, 0);
    }

    #[test]
    fn test_simple_branch_and_merge() {
        // Timeline (newest first):
        // merge (parents: feat, c2) -> feat (parent: c1) -> c2 (parent: c1) -> c1
        let commits = vec![
            make_commit("merge", vec!["c2", "feat"], vec!["HEAD -> main"]),
            make_commit("c2", vec!["c1"], vec![]),
            make_commit("feat", vec!["c1"], vec!["feature"]),
            make_commit("c1", vec![], vec![]),
        ];
        let layout = compute_graph_layout(&commits);
        assert_eq!(layout.nodes.len(), 4);

        // merge on lane 0
        assert_eq!(layout.nodes[0].lane, 0);
        // c2 inherits lane 0 (first parent of merge)
        assert_eq!(layout.nodes[1].lane, 0);

        // feat gets a separate lane
        let feat_lane = layout.nodes[2].lane;
        assert!(feat_lane > 0, "feature branch should be on lane > 0");

        // Edges should exist
        assert!(layout.edges.len() >= 3);
    }

    #[test]
    fn test_multiple_branches() {
        // c5 (merge of c4 and c3)
        // c4 (from c1, branch A)
        // c3 (from c1, branch B)
        // c2 (from c1, main line)
        // c1 (root)
        let commits = vec![
            make_commit("c5", vec!["c2", "c4", "c3"], vec!["HEAD -> main"]),
            make_commit("c4", vec!["c1"], vec!["branchA"]),
            make_commit("c3", vec!["c1"], vec!["branchB"]),
            make_commit("c2", vec!["c1"], vec![]),
            make_commit("c1", vec![], vec![]),
        ];
        let layout = compute_graph_layout(&commits);
        assert_eq!(layout.nodes.len(), 5);

        // Main merge on lane 0
        assert_eq!(layout.nodes[0].lane, 0);

        // Should use at least 2 different lanes for the branches
        let lanes: HashSet<u32> = layout.nodes.iter().map(|n| n.lane).collect();
        assert!(lanes.len() >= 2, "Should use multiple lanes for branches");
    }

    #[test]
    fn test_octopus_merge() {
        // A commit with 3 parents (octopus merge)
        let commits = vec![
            make_commit("octopus", vec!["p1", "p2", "p3"], vec!["HEAD -> main"]),
            make_commit("p1", vec!["root"], vec![]),
            make_commit("p2", vec!["root"], vec![]),
            make_commit("p3", vec!["root"], vec![]),
            make_commit("root", vec![], vec![]),
        ];
        let layout = compute_graph_layout(&commits);
        assert_eq!(layout.nodes.len(), 5);

        // Should have edges from octopus to all 3 parents
        let octopus_edges: Vec<_> = layout
            .edges
            .iter()
            .filter(|e| e.from_sha == "octopus")
            .collect();
        assert_eq!(octopus_edges.len(), 3, "Octopus merge should have 3 edges to parents");
    }

    #[test]
    fn test_graph_node_rows_are_sequential() {
        let commits = vec![
            make_commit("c3", vec!["c2"], vec![]),
            make_commit("c2", vec!["c1"], vec![]),
            make_commit("c1", vec![], vec![]),
        ];
        let layout = compute_graph_layout(&commits);
        for (i, node) in layout.nodes.iter().enumerate() {
            assert_eq!(node.row, i as u32, "Row should match index");
        }
    }

    #[test]
    fn test_edges_reference_valid_shas() {
        let commits = vec![
            make_commit("c2", vec!["c1"], vec![]),
            make_commit("c1", vec![], vec![]),
        ];
        let layout = compute_graph_layout(&commits);
        let shas: HashSet<&str> = commits.iter().map(|c| c.sha.as_str()).collect();
        for edge in &layout.edges {
            assert!(shas.contains(edge.from_sha.as_str()));
            assert!(shas.contains(edge.to_sha.as_str()));
        }
    }

    #[test]
    fn test_find_min_available_lane() {
        let mut active = HashSet::new();
        assert_eq!(find_min_available_lane(&active), 0);

        active.insert(0);
        assert_eq!(find_min_available_lane(&active), 1);

        active.insert(1);
        assert_eq!(find_min_available_lane(&active), 2);

        active.remove(&0);
        assert_eq!(find_min_available_lane(&active), 0);
    }

    #[test]
    fn test_long_linear_history() {
        let mut commits = Vec::new();
        for i in 0..100 {
            let sha = format!("commit_{:03}", i);
            let parent = if i < 99 {
                vec![format!("commit_{:03}", i + 1)]
            } else {
                vec![]
            };
            commits.push(CommitNode {
                sha: sha.clone(),
                short_sha: sha[..10].to_string(),
                parents: parent,
                author_name: "Test".to_string(),
                author_email: "test@test.com".to_string(),
                date: "2026-02-19".to_string(),
                message: format!("commit {}", i),
                refs: vec![],
            });
        }
        let layout = compute_graph_layout(&commits);
        assert_eq!(layout.nodes.len(), 100);
        assert_eq!(layout.max_lane, 0, "Linear history should stay on lane 0");
        assert_eq!(layout.edges.len(), 99);
    }

    #[test]
    fn test_parent_outside_commit_set() {
        // Parent SHA not in the commit list (truncated log)
        let commits = vec![
            make_commit("c2", vec!["c1"], vec![]),
            make_commit("c1", vec!["missing_parent"], vec![]),
        ];
        let layout = compute_graph_layout(&commits);
        assert_eq!(layout.nodes.len(), 2);
        // Only one edge (c2->c1), no edge to missing_parent
        assert_eq!(layout.edges.len(), 1);
        assert_eq!(layout.edges[0].from_sha, "c2");
        assert_eq!(layout.edges[0].to_sha, "c1");
    }
}
