use git2::{Commit, Oid, Repository, Sort};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Git graph node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    /// Commit hash
    pub hash: String,
    /// Commit message (first line)
    pub message: String,
    /// Full commit message
    pub full_message: String,
    /// Author name
    pub author_name: String,
    /// Author email
    pub author_email: String,
    /// Commit time (Unix timestamp)
    pub timestamp: i64,
    /// Parent commit hashes
    pub parents: Vec<String>,
    /// Child commit hashes (filled when building the graph)
    pub children: Vec<String>,
    /// Associated refs (branches, tags, etc.)
    pub refs: Vec<GraphRef>,
    /// Lane position
    pub lane: i32,
    /// Lanes that fork out
    pub forking_lanes: Vec<i32>,
    /// Lanes that merge in
    pub merging_lanes: Vec<i32>,
    /// Lanes that pass through
    pub passing_lanes: Vec<i32>,
}

/// Git ref information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRef {
    /// Ref name
    pub name: String,
    /// Ref type: `branch`, `remote`, `tag`
    pub ref_type: String,
    /// Whether this is the current branch
    pub is_current: bool,
    /// Whether this is `HEAD`
    pub is_head: bool,
}

/// Git graph data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraph {
    /// Node list
    pub nodes: Vec<GraphNode>,
    /// Maximum lane count
    pub max_lane: i32,
    /// Current branch name
    pub current_branch: Option<String>,
}

/// Lane allocator
struct LaneAllocator {
    /// Active lanes: lane position -> commit hash
    active_lanes: HashMap<i32, String>,
    /// Free lane positions
    free_positions: Vec<i32>,
    /// Next available position
    next_position: i32,
    /// Lane length stats
    lane_lengths: HashMap<i32, usize>,
}

impl LaneAllocator {
    fn new() -> Self {
        Self {
            active_lanes: HashMap::new(),
            free_positions: Vec::new(),
            next_position: 0,
            lane_lengths: HashMap::new(),
        }
    }

    /// Allocates a new lane.
    fn allocate(&mut self, commit_hash: String) -> i32 {
        let position = if let Some(pos) = self.free_positions.pop() {
            pos
        } else {
            let pos = self.next_position;
            self.next_position += 1;
            pos
        };

        self.active_lanes.insert(position, commit_hash);
        self.lane_lengths.insert(position, 1);
        position
    }

    /// Frees a lane.
    fn free(&mut self, position: i32) {
        self.active_lanes.remove(&position);
        self.lane_lengths.remove(&position);
        self.free_positions.push(position);
        self.free_positions.sort_unstable();
    }

    /// Increments the lane length.
    fn increment_length(&mut self, position: i32) {
        if let Some(len) = self.lane_lengths.get_mut(&position) {
            *len += 1;
        }
    }

    /// Returns the lane length.
    fn get_length(&self, position: i32) -> usize {
        self.lane_lengths.get(&position).copied().unwrap_or(0)
    }
}

/// Builds a Git graph.
pub fn build_git_graph(
    repo: &Repository,
    max_count: Option<usize>,
) -> Result<GitGraph, git2::Error> {
    build_git_graph_for_branch(repo, max_count, None)
}

/// Builds a Git graph for a specific branch.
pub fn build_git_graph_for_branch(
    repo: &Repository,
    max_count: Option<usize>,
    branch_name: Option<&str>,
) -> Result<GitGraph, git2::Error> {
    let current_branch = get_current_branch(repo);

    let refs_map = collect_refs(repo)?;

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;

    if let Some(branch) = branch_name {
        if let Ok(reference) = repo.find_branch(branch, git2::BranchType::Local) {
            if let Some(oid) = reference.get().target() {
                revwalk.push(oid)?;
            }
        } else if let Ok(reference) = repo.find_branch(branch, git2::BranchType::Remote) {
            if let Some(oid) = reference.get().target() {
                revwalk.push(oid)?;
            }
        } else if let Ok(reference) = repo.find_reference(&format!("refs/heads/{}", branch)) {
            if let Some(oid) = reference.target() {
                revwalk.push(oid)?;
            }
        } else {
            for reference in repo.references()? {
                let reference = reference?;
                if reference.is_branch() || reference.is_remote() || reference.is_tag() {
                    if let Some(oid) = reference.target() {
                        revwalk.push(oid)?;
                    }
                }
            }
        }
    } else {
        for reference in repo.references()? {
            let reference = reference?;
            if reference.is_branch() || reference.is_remote() || reference.is_tag() {
                if let Some(oid) = reference.target() {
                    revwalk.push(oid)?;
                }
            }
        }
    }

    let mut commits: Vec<(Oid, Commit)> = Vec::new();
    let max_count = max_count.unwrap_or(1000);

    for oid_result in revwalk.take(max_count) {
        let oid = oid_result?;
        if let Ok(commit) = repo.find_commit(oid) {
            commits.push((oid, commit));
        }
    }

    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    for (oid, commit) in &commits {
        let hash = oid.to_string();
        for parent_id in commit.parent_ids() {
            let parent_hash = parent_id.to_string();
            children_map
                .entry(parent_hash)
                .or_default()
                .push(hash.clone());
        }
    }

    let mut nodes: Vec<GraphNode> = Vec::new();
    for (oid, commit) in commits {
        let hash = oid.to_string();
        let message = commit.summary().unwrap_or("").to_string();
        let full_message = commit.message().unwrap_or("").to_string();
        let author = commit.author();

        let node = GraphNode {
            hash: hash.clone(),
            message,
            full_message,
            author_name: author.name().unwrap_or("Unknown").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            timestamp: author.when().seconds(),
            parents: commit.parent_ids().map(|id| id.to_string()).collect(),
            children: children_map.get(&hash).cloned().unwrap_or_default(),
            refs: refs_map.get(&hash).cloned().unwrap_or_default(),
            lane: -1,
            forking_lanes: Vec::new(),
            merging_lanes: Vec::new(),
            passing_lanes: Vec::new(),
        };

        nodes.push(node);
    }

    let max_lane = allocate_lanes(&mut nodes);

    Ok(GitGraph {
        nodes,
        max_lane,
        current_branch,
    })
}

/// Collects all refs.
fn collect_refs(repo: &Repository) -> Result<HashMap<String, Vec<GraphRef>>, git2::Error> {
    let mut refs_map: HashMap<String, Vec<GraphRef>> = HashMap::new();
    let head = repo.head().ok();
    let current_branch = get_current_branch(repo);

    for reference in repo.references()? {
        let reference = reference?;

        let (ref_type, name) = if reference.is_branch() {
            ("branch", reference.shorthand().unwrap_or(""))
        } else if reference.is_remote() {
            ("remote", reference.shorthand().unwrap_or(""))
        } else if reference.is_tag() {
            ("tag", reference.shorthand().unwrap_or(""))
        } else {
            continue;
        };

        if let Some(oid) = reference.target() {
            let hash = oid.to_string();
            let is_current = current_branch.as_ref().is_some_and(|cb| cb == name);
            let is_head = head.as_ref().and_then(|h| h.target()) == Some(oid);

            let graph_ref = GraphRef {
                name: name.to_string(),
                ref_type: ref_type.to_string(),
                is_current,
                is_head,
            };

            refs_map.entry(hash).or_default().push(graph_ref);
        }
    }

    Ok(refs_map)
}

/// Returns the current branch name.
fn get_current_branch(repo: &Repository) -> Option<String> {
    repo.head()
        .ok()
        .and_then(|head| head.shorthand().map(|s| s.to_string()))
}

/// Allocates lanes (simplified algorithm).
fn allocate_lanes(nodes: &mut [GraphNode]) -> i32 {
    if nodes.is_empty() {
        return 0;
    }

    let mut allocator = LaneAllocator::new();
    let mut commit_lanes: HashMap<String, i32> = HashMap::new();

    for node in nodes.iter_mut() {
        let hash = node.hash.clone();

        let lane = if node.children.is_empty() {
            allocator.allocate(hash.clone())
        } else if node.children.len() == 1 {
            let child_hash = &node.children[0];
            if let Some(&child_lane) = commit_lanes.get(child_hash) {
                allocator.increment_length(child_lane);
                child_lane
            } else {
                allocator.allocate(hash.clone())
            }
        } else {
            let mut best_lane = -1;
            let mut best_length = 0;

            for child_hash in &node.children {
                if let Some(&child_lane) = commit_lanes.get(child_hash) {
                    let length = allocator.get_length(child_lane);
                    if length > best_length {
                        best_length = length;
                        best_lane = child_lane;
                    }
                }
            }

            if best_lane >= 0 {
                allocator.increment_length(best_lane);
                best_lane
            } else {
                allocator.allocate(hash.clone())
            }
        };

        node.lane = lane;
        commit_lanes.insert(hash.clone(), lane);

        for child_hash in &node.children {
            if let Some(&child_lane) = commit_lanes.get(child_hash) {
                if child_lane != lane {
                    node.forking_lanes.push(child_lane);
                }
            }
        }

        for (i, parent_hash) in node.parents.iter().enumerate() {
            if i > 0 {
                if let Some(&parent_lane) = commit_lanes.get(parent_hash) {
                    if parent_lane != lane {
                        node.merging_lanes.push(parent_lane);
                    }
                }
            }
        }

        let active_lanes: Vec<i32> = allocator.active_lanes.keys().copied().collect();
        for &active_lane in &active_lanes {
            if active_lane != lane
                && !node.forking_lanes.contains(&active_lane)
                && !node.merging_lanes.contains(&active_lane)
            {
                node.passing_lanes.push(active_lane);
            }
        }

        if node.parents.is_empty() {
            allocator.free(lane);
        }
    }

    allocator.next_position
}
