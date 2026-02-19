//! Commit DAG Layout Algorithm — "Railroad Track" style
//!
//! Produces compact, professional-quality graph layouts similar to
//! Fork / GitKraken / VS Code Git Graph.
//!
//! Core idea: maintain a list of **active columns** where each column
//! tracks the next expected SHA. Process commits newest-first:
//!
//! 1. Find which column(s) are waiting for this commit's SHA.
//! 2. The commit's lane = leftmost matching column (or first empty slot
//!    for new branch tips).
//! 3. First parent inherits the column; additional parents open new columns.
//! 4. Duplicate columns (merge) are closed immediately.
//! 5. Trailing empty columns are trimmed every iteration for compactness.
//!
//! Two-pass design: pass 1 assigns lanes, pass 2 builds edges from the
//! final lane assignments so that edge endpoints are always correct.

use std::collections::HashMap;

use super::types::{CommitNode, GraphEdge, GraphLayout, GraphNode};

/// Compute graph layout from a topologically-ordered list of commits.
///
/// The input `commits` must be ordered newest-first (as `git log` outputs).
pub fn compute_graph_layout(commits: &[CommitNode]) -> GraphLayout {
    if commits.is_empty() {
        return GraphLayout::default();
    }

    let sha_to_idx: HashMap<&str, usize> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| (c.sha.as_str(), i))
        .collect();

    // Active columns: each entry is the SHA this column is leading to.
    let mut columns: Vec<Option<String>> = Vec::new();

    // Per-commit lane assignment (indexed by commit position).
    let mut lane_of: Vec<u32> = vec![0; commits.len()];
    let mut max_lane: u32 = 0;

    // -----------------------------------------------------------------------
    // Pass 1 — assign every commit to a lane
    // -----------------------------------------------------------------------
    for (idx, commit) in commits.iter().enumerate() {
        let sha = commit.sha.as_str();

        // Which column(s) are waiting for this SHA?
        let matching: Vec<usize> = columns
            .iter()
            .enumerate()
            .filter_map(|(i, c)| match c {
                Some(s) if s.as_str() == sha => Some(i),
                _ => None,
            })
            .collect();

        // Determine lane
        let lane = if matching.is_empty() {
            // New branch tip — first empty slot or append
            let col = first_empty_or_new(&mut columns);
            col
        } else {
            // Use leftmost column that was waiting for us
            matching[0]
        };

        lane_of[idx] = lane as u32;
        if (lane as u32) > max_lane {
            max_lane = lane as u32;
        }

        // Close extra columns (commit found in >1 column due to merges)
        for &c in matching.iter().skip(1) {
            columns[c] = None;
        }
        // Also clear the lane itself (will be re-set by parent logic below)
        columns[lane] = None;

        // Parents visible in this commit window
        let parents: Vec<&str> = commit
            .parents
            .iter()
            .map(|p| p.as_str())
            .filter(|p| sha_to_idx.contains_key(p))
            .collect();

        if parents.is_empty() {
            // Root commit — lane stays free (already cleared above)
        } else {
            // First parent inherits this column
            let first_parent = parents[0];
            columns[lane] = Some(first_parent.to_string());

            // If first parent was already tracked in a different column
            // (because another path led to it), resolve the duplicate by
            // keeping the lower-indexed column (keeps main branch compact).
            for i in 0..columns.len() {
                if i != lane && columns[i].as_deref() == Some(first_parent) {
                    if i < lane {
                        // Other column is lower — keep it there, clear ours
                        columns[lane] = None;
                    } else {
                        // Our column is lower — clear the other
                        columns[i] = None;
                    }
                }
            }

            // Additional parents (merge sources) open new columns
            for &p in parents.iter().skip(1) {
                let already = columns.iter().any(|c| c.as_deref() == Some(p));
                if !already {
                    let col = first_empty_or_new(&mut columns);
                    columns[col] = Some(p.to_string());
                    if (col as u32) > max_lane {
                        max_lane = col as u32;
                    }
                }
            }
        }

        // Compact: trim trailing empty slots
        while columns.last().map_or(false, |c| c.is_none()) {
            columns.pop();
        }
    }

    // -----------------------------------------------------------------------
    // Build nodes
    // -----------------------------------------------------------------------
    let nodes: Vec<GraphNode> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| GraphNode {
            sha: c.sha.clone(),
            row: i as u32,
            lane: lane_of[i],
        })
        .collect();

    // -----------------------------------------------------------------------
    // Pass 2 — build edges using final lane assignments
    // -----------------------------------------------------------------------
    let mut edges = Vec::new();
    for (idx, commit) in commits.iter().enumerate() {
        for parent_sha in &commit.parents {
            if let Some(&pidx) = sha_to_idx.get(parent_sha.as_str()) {
                edges.push(GraphEdge {
                    from_sha: commit.sha.clone(),
                    to_sha: parent_sha.clone(),
                    from_lane: lane_of[idx],
                    to_lane: lane_of[pidx],
                    from_row: idx as u32,
                    to_row: pidx as u32,
                });
            }
        }
    }

    GraphLayout {
        nodes,
        edges,
        max_lane,
    }
}

/// Return index of first `None` slot, or append a new slot and return its index.
fn first_empty_or_new(columns: &mut Vec<Option<String>>) -> usize {
    if let Some(pos) = columns.iter().position(|c| c.is_none()) {
        pos
    } else {
        columns.push(None);
        columns.len() - 1
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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
        assert!(layout.edges.is_empty());
    }

    #[test]
    fn test_linear_history() {
        let commits = vec![
            make_commit("ccc", vec!["bbb"], vec!["HEAD -> main"]),
            make_commit("bbb", vec!["aaa"], vec![]),
            make_commit("aaa", vec![], vec![]),
        ];
        let layout = compute_graph_layout(&commits);
        // All on lane 0
        for node in &layout.nodes {
            assert_eq!(node.lane, 0);
        }
        assert_eq!(layout.edges.len(), 2);
        assert_eq!(layout.max_lane, 0);
    }

    #[test]
    fn test_simple_branch_and_merge() {
        // merge (first-parent: c2, second-parent: feat)
        // c2 (parent: c1)
        // feat (parent: c1)
        // c1 (root)
        let commits = vec![
            make_commit("merge", vec!["c2", "feat"], vec!["HEAD -> main"]),
            make_commit("c2", vec!["c1"], vec![]),
            make_commit("feat", vec!["c1"], vec!["feature"]),
            make_commit("c1", vec![], vec![]),
        ];
        let layout = compute_graph_layout(&commits);

        // merge on lane 0, c2 inherits lane 0 (first parent)
        assert_eq!(layout.nodes[0].lane, 0);
        assert_eq!(layout.nodes[1].lane, 0);

        // feat on separate lane
        assert!(layout.nodes[2].lane > 0);

        // c1 back on lane 0 (first parent of c2)
        assert_eq!(layout.nodes[3].lane, 0);
    }

    #[test]
    fn test_lane_reuse_after_merge() {
        // After a branch merges, its lane should be reused.
        // m2 (merge b2 into main) -> m1 (merge b1 into main) -> ...
        // b2 branches from base2, b1 branches from base1
        let commits = vec![
            make_commit("m2", vec!["main2", "b2"], vec!["HEAD"]),
            make_commit("main2", vec!["m1"], vec![]),
            make_commit("b2", vec!["base2"], vec![]),
            make_commit("m1", vec!["base2", "b1"], vec![]),
            make_commit("base2", vec!["b1_base"], vec![]),
            make_commit("b1", vec!["b1_base"], vec![]),
            make_commit("b1_base", vec![], vec![]),
        ];
        let layout = compute_graph_layout(&commits);

        // b1 and b2 should reuse lane 1 (not spread to lane 2)
        let b2_lane = layout.nodes[2].lane;
        let b1_lane = layout.nodes[5].lane;
        assert_eq!(b2_lane, 1, "b2 should be on lane 1");
        assert_eq!(b1_lane, 1, "b1 should reuse lane 1 after b2 merged");
        assert!(layout.max_lane <= 1, "max_lane should be 1 with lane reuse");
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
        assert_eq!(layout.max_lane, 0, "Linear history should stay on lane 0");
    }

    #[test]
    fn test_multiple_branches() {
        let commits = vec![
            make_commit("c5", vec!["c2", "c4", "c3"], vec!["HEAD -> main"]),
            make_commit("c4", vec!["c1"], vec!["branchA"]),
            make_commit("c3", vec!["c1"], vec!["branchB"]),
            make_commit("c2", vec!["c1"], vec![]),
            make_commit("c1", vec![], vec![]),
        ];
        let layout = compute_graph_layout(&commits);
        let lanes: HashSet<u32> = layout.nodes.iter().map(|n| n.lane).collect();
        assert!(lanes.len() >= 2, "Should use multiple lanes for branches");
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
    fn test_parent_outside_commit_set() {
        let commits = vec![
            make_commit("c2", vec!["c1"], vec![]),
            make_commit("c1", vec!["missing_parent"], vec![]),
        ];
        let layout = compute_graph_layout(&commits);
        assert_eq!(layout.edges.len(), 1);
    }
}
