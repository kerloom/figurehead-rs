//! State diagram layout algorithm
//!
//! Positions states and transitions for rendering.

use super::database::{NoteSide, StateDatabase, START_TERMINAL};
use crate::core::{wrap_label, LayoutAlgorithm, NodeShape};
use anyhow::Result;
use std::collections::{HashMap, HashSet, VecDeque};

pub const MAX_TRANSITION_LABEL_WIDTH: usize = 28;

fn transition_label_width(label: &str) -> usize {
    wrap_label(label, MAX_TRANSITION_LABEL_WIDTH)
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
}

/// Per-rank state dimensions, maximum height, and total row width.
type RankInfo = (Vec<(usize, usize, usize, usize)>, usize, usize);

/// Positioned state for rendering
#[derive(Debug, Clone)]
pub struct PositionedState {
    pub id: String,
    pub label: String,
    pub shape: NodeShape,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub rank: usize,
}

/// Positioned transition for rendering
#[derive(Debug, Clone)]
pub struct PositionedTransition {
    pub from_id: String,
    pub to_id: String,
    pub label: Option<String>,
    pub from_x: usize,
    pub from_y: usize,
    pub to_x: usize,
    pub to_y: usize,
    pub route: TransitionRoute,
    pub lane: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionRoute {
    Forward,
    Backward,
    Horizontal,
    SelfLoop,
}

#[derive(Debug, Clone)]
pub struct PositionedNote {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub text: Vec<String>,
}

/// Layout result containing positioned elements
#[derive(Debug, Clone)]
pub struct StateLayoutResult {
    pub states: Vec<PositionedState>,
    pub transitions: Vec<PositionedTransition>,
    pub notes: Vec<PositionedNote>,
    pub width: usize,
    pub height: usize,
}

/// State diagram layout algorithm
pub struct StateLayoutAlgorithm {
    /// Minimum state box width
    min_state_width: usize,
    /// State box height (for normal states)
    state_height: usize,
    /// Terminal state size
    terminal_size: usize,
    /// Horizontal spacing between states
    h_spacing: usize,
    /// Vertical spacing between ranks
    v_spacing: usize,
    /// Padding around labels
    padding: usize,
}

impl StateLayoutAlgorithm {
    pub fn new() -> Self {
        Self {
            min_state_width: 8,
            state_height: 3,
            terminal_size: 3,
            h_spacing: 3,
            v_spacing: 7,
            padding: 2,
        }
    }

    /// Assign ranks to states using BFS from start state
    fn assign_ranks(&self, db: &StateDatabase) -> HashMap<String, usize> {
        let mut ranks: HashMap<String, usize> = HashMap::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();

        // Find start terminal or first state
        let start = if db.has_start_terminal() {
            START_TERMINAL.to_string()
        } else if let Some(first) = db.states().first() {
            first.id.clone()
        } else {
            return ranks;
        };

        // BFS from start
        queue.push_back((start, 0));

        while let Some((state_id, rank)) = queue.pop_front() {
            if visited.contains(&state_id) {
                continue;
            }
            visited.insert(state_id.clone());
            ranks.insert(state_id.clone(), rank);

            // Add all states reachable from this one
            for edge in db.transitions() {
                if edge.from == state_id && !visited.contains(&edge.to) {
                    queue.push_back((edge.to.clone(), rank + 1));
                }
            }
        }

        // Handle any unvisited states (disconnected)
        let max_rank = ranks.values().copied().max().unwrap_or(0);
        for state in db.states() {
            if !ranks.contains_key(&state.id) {
                ranks.insert(state.id.clone(), max_rank + 1);
            }
        }

        ranks
    }

    /// Calculate state dimensions
    fn calculate_state_size(&self, label: &str, shape: NodeShape) -> (usize, usize) {
        match shape {
            NodeShape::Terminal => (self.terminal_size, self.terminal_size),
            _ => {
                let label_width = label.chars().count();
                let width = (label_width + self.padding * 2).max(self.min_state_width);
                (width, self.state_height)
            }
        }
    }

    /// Layout the database
    pub fn layout(&self, db: &StateDatabase) -> Result<StateLayoutResult> {
        if db.state_count() == 0 {
            return Ok(StateLayoutResult {
                states: vec![],
                transitions: vec![],
                notes: vec![],
                width: 0,
                height: 0,
            });
        }

        let ranks = self.assign_ranks(db);
        let backward_edges: Vec<_> = db
            .transitions()
            .iter()
            .filter(|edge| {
                edge.from != edge.to
                    && ranks.get(&edge.to).copied().unwrap_or(0)
                        < ranks.get(&edge.from).copied().unwrap_or(0)
            })
            .collect();
        let max_backward_label_width = backward_edges
            .iter()
            .filter_map(|edge| edge.label.as_ref())
            .map(|label| transition_label_width(label))
            .max()
            .unwrap_or(0);
        let max_left_note_width = db
            .notes()
            .iter()
            .filter(|note| note.side == NoteSide::Left)
            .flat_map(|note| note.text.iter())
            .map(|line| line.chars().count() + 2)
            .max()
            .unwrap_or(0);
        let left_margin =
            backward_edges.len() * 2 + max_backward_label_width + max_left_note_width + 8;

        // Group states by rank
        let mut by_rank: HashMap<usize, Vec<&crate::core::NodeData>> = HashMap::new();
        for state in db.states() {
            let rank = ranks.get(&state.id).copied().unwrap_or(0);
            by_rank.entry(rank).or_default().push(state);
        }

        let max_rank = *ranks.values().max().unwrap_or(&0);

        // First pass: calculate dimensions and find max row width
        let mut rank_info: Vec<RankInfo> = Vec::new();
        let mut max_row_width = 0;

        for rank in 0..=max_rank {
            let states_in_rank = by_rank.get(&rank).map(|v| v.as_slice()).unwrap_or(&[]);

            if states_in_rank.is_empty() {
                rank_info.push((vec![], 0, 0));
                continue;
            }

            let mut max_height = 0;
            let mut state_dims: Vec<(usize, usize, usize, usize)> = Vec::new();
            let mut row_width = 0;

            for (i, state) in states_in_rank.iter().enumerate() {
                let (w, h) = self.calculate_state_size(&state.label, state.shape);
                let connected_label_width = db
                    .transitions()
                    .iter()
                    .filter(|edge| {
                        edge.from != edge.to && (edge.from == state.id || edge.to == state.id)
                    })
                    .filter_map(|edge| edge.label.as_ref())
                    .map(|label| transition_label_width(label) + 4)
                    .max()
                    .unwrap_or(0);
                let self_label_width = db
                    .transitions()
                    .iter()
                    .filter(|edge| edge.from == state.id && edge.to == state.id)
                    .filter_map(|edge| edge.label.as_ref())
                    .map(|label| transition_label_width(label))
                    .max()
                    .unwrap_or(0);
                let slot_width = w
                    .max(connected_label_width)
                    .max(w + self_label_width + usize::from(self_label_width > 0) * 8);
                let state_offset = if self_label_width > 0 {
                    0
                } else {
                    (slot_width - w) / 2
                };
                state_dims.push((w, h, slot_width, state_offset));
                max_height = max_height.max(h);
                row_width += slot_width;
                if i > 0 {
                    row_width += self.h_spacing;
                }
            }

            max_row_width = max_row_width.max(row_width);
            rank_info.push((state_dims, max_height, row_width));
        }

        // The center line for the entire diagram
        let center_x = max_row_width / 2;

        // Second pass: position states with centers aligned
        let mut positioned_states: Vec<PositionedState> = Vec::new();
        let mut state_positions: HashMap<String, (usize, usize, usize, usize)> = HashMap::new();
        let mut current_y = 0;

        for (rank, (ref state_dims, max_height, row_width)) in rank_info.iter().enumerate() {
            let states_in_rank = by_rank.get(&rank).map(|v| v.as_slice()).unwrap_or(&[]);

            if states_in_rank.is_empty() {
                continue;
            }

            // Center this row on center_x
            let row_start = left_margin + center_x.saturating_sub(row_width / 2);
            let mut current_x = row_start;

            for (i, state) in states_in_rank.iter().enumerate() {
                let (w, h, slot_width, state_offset) = state_dims[i];
                let y_offset = (max_height - h) / 2;
                let state_x = current_x + state_offset;

                let pos_state = PositionedState {
                    id: state.id.clone(),
                    label: state.label.clone(),
                    shape: state.shape,
                    x: state_x,
                    y: current_y + y_offset,
                    width: w,
                    height: h,
                    rank,
                };

                state_positions.insert(state.id.clone(), (state_x, current_y + y_offset, w, h));
                positioned_states.push(pos_state);

                current_x += slot_width + self.h_spacing;
            }

            current_y += max_height + self.v_spacing;
        }

        // Position transitions
        let mut positioned_transitions: Vec<PositionedTransition> = Vec::new();

        let mut backward_lane = 0;
        let mut self_lane = 0;
        for edge in db.transitions() {
            if let (Some(&(fx, fy, fw, fh)), Some(&(tx, ty, tw, th))) = (
                state_positions.get(&edge.from),
                state_positions.get(&edge.to),
            ) {
                let from_rank = ranks.get(&edge.from).copied().unwrap_or(0);
                let to_rank = ranks.get(&edge.to).copied().unwrap_or(0);
                let (from_x, from_y, to_x, to_y, route, lane) = if edge.from == edge.to {
                    let lane = self_lane;
                    self_lane += 1;
                    (
                        fx + fw,
                        fy + fh / 2,
                        fx + fw / 2,
                        fy + fh,
                        TransitionRoute::SelfLoop,
                        lane,
                    )
                } else if to_rank > from_rank {
                    (
                        fx + fw / 2,
                        fy + fh,
                        tx + tw / 2,
                        ty,
                        TransitionRoute::Forward,
                        0,
                    )
                } else if to_rank < from_rank {
                    let lane = backward_lane;
                    backward_lane += 1;
                    (
                        fx,
                        fy + fh / 2,
                        tx,
                        ty + th / 2,
                        TransitionRoute::Backward,
                        lane,
                    )
                } else if fx < tx {
                    (
                        fx + fw,
                        fy + fh / 2,
                        tx,
                        ty + th / 2,
                        TransitionRoute::Horizontal,
                        0,
                    )
                } else {
                    (
                        fx,
                        fy + fh / 2,
                        tx + tw,
                        ty + th / 2,
                        TransitionRoute::Horizontal,
                        0,
                    )
                };

                positioned_transitions.push(PositionedTransition {
                    from_id: edge.from.clone(),
                    to_id: edge.to.clone(),
                    label: edge.label.clone(),
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    route,
                    lane,
                });
            }
        }

        let positioned_notes: Vec<PositionedNote> = db
            .notes()
            .iter()
            .filter_map(|note| {
                let &(x, y, width, _) = state_positions.get(&note.state_id)?;
                let note_width = note
                    .text
                    .iter()
                    .map(|line| line.chars().count() + 2)
                    .max()
                    .unwrap_or(4)
                    .max(4);
                let self_label_width = db
                    .transitions()
                    .iter()
                    .filter(|edge| edge.from == note.state_id && edge.to == note.state_id)
                    .filter_map(|edge| edge.label.as_ref())
                    .map(|label| transition_label_width(label))
                    .max()
                    .unwrap_or(0);
                let x = match note.side {
                    NoteSide::Left => x.saturating_sub(note_width + 2),
                    NoteSide::Right => x + width + self_label_width + 8,
                };
                Some(PositionedNote {
                    x,
                    y,
                    width: note_width,
                    text: note.text.clone(),
                })
            })
            .collect();

        // Calculate total dimensions
        let mut width = positioned_states
            .iter()
            .map(|s| s.x + s.width)
            .max()
            .unwrap_or(0);
        width = width.max(
            positioned_notes
                .iter()
                .map(|note| note.x + note.width)
                .max()
                .unwrap_or(0),
        );
        width = width.max(
            positioned_transitions
                .iter()
                .map(|edge| {
                    edge.from_x
                        + edge
                            .label
                            .as_ref()
                            .map_or(0, |label| transition_label_width(label))
                        + edge.lane * 2
                        + 6
                })
                .max()
                .unwrap_or(0),
        );
        width += self_lane * 2 + 4;
        let mut height = positioned_states
            .iter()
            .map(|s| s.y + s.height)
            .max()
            .unwrap_or(0);
        height = height.max(
            positioned_notes
                .iter()
                .map(|note| note.y + note.text.len() + 2)
                .max()
                .unwrap_or(0),
        );

        Ok(StateLayoutResult {
            states: positioned_states,
            transitions: positioned_transitions,
            notes: positioned_notes,
            width,
            height,
        })
    }
}

impl Default for StateLayoutAlgorithm {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutAlgorithm<StateDatabase> for StateLayoutAlgorithm {
    type Output = StateLayoutResult;

    fn layout(&self, database: &StateDatabase) -> Result<Self::Output> {
        self.layout(database)
    }

    fn name(&self) -> &'static str {
        "state"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn direction(&self) -> &'static str {
        "TB"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EdgeData;

    #[test]
    fn test_empty_layout() {
        let db = StateDatabase::new();
        let algo = StateLayoutAlgorithm::new();
        let result = algo.layout(&db).unwrap();

        assert!(result.states.is_empty());
        assert!(result.transitions.is_empty());
    }

    #[test]
    fn test_single_state_layout() {
        let mut db = StateDatabase::new();
        db.add_state(crate::core::NodeData::new("Idle", "Idle"))
            .unwrap();

        let algo = StateLayoutAlgorithm::new();
        let result = algo.layout(&db).unwrap();

        assert_eq!(result.states.len(), 1);
        assert_eq!(result.states[0].id, "Idle");
    }

    #[test]
    fn test_linear_layout() {
        let mut db = StateDatabase::new();
        db.add_transition(EdgeData::new("[*]", "Idle")).unwrap();
        db.add_transition(EdgeData::new("Idle", "Running")).unwrap();
        db.add_transition(EdgeData::new("Running", "[*]")).unwrap();

        let algo = StateLayoutAlgorithm::new();
        let result = algo.layout(&db).unwrap();

        // Should have 3 states (2x [*] collapsed + Idle + Running)
        assert!(result.states.len() >= 3);

        // States should be in different ranks (y positions)
        let y_positions: Vec<usize> = result.states.iter().map(|s| s.y).collect();
        // Multiple y positions means vertical layout
        assert!(y_positions.iter().collect::<HashSet<_>>().len() > 1);
    }

    #[test]
    fn test_terminal_state_size() {
        let algo = StateLayoutAlgorithm::new();
        let (w, h) = algo.calculate_state_size("", NodeShape::Terminal);
        assert_eq!(w, algo.terminal_size);
        assert_eq!(h, algo.terminal_size);
    }

    #[test]
    fn test_branching_layout() {
        let mut db = StateDatabase::new();
        db.add_transition(EdgeData::new("[*]", "Idle")).unwrap();
        db.add_transition(EdgeData::new("Idle", "Processing"))
            .unwrap();
        db.add_transition(EdgeData::new("Processing", "Success"))
            .unwrap();
        db.add_transition(EdgeData::new("Processing", "Failed"))
            .unwrap();
        db.add_transition(EdgeData::new("Success", "[*]")).unwrap();
        db.add_transition(EdgeData::new("Failed", "[*]")).unwrap();

        let algo = StateLayoutAlgorithm::new();
        let result = algo.layout(&db).unwrap();

        // Find the start terminal
        let start = result
            .states
            .iter()
            .find(|s| s.id == START_TERMINAL)
            .unwrap();

        // The start terminal should be horizontally centered
        // It should NOT be at x=0 for a branching diagram
        assert!(start.x > 0, "Start terminal x={} should be > 0", start.x);
    }
}
