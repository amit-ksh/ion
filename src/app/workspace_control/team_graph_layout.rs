//! In-memory tree layout. Parent indices retain the team's existing member order.
pub(super) const NODE_WIDTH: f32 = 216.0;
pub(super) const NODE_HEIGHT: f32 = 68.0;
const GAP: f32 = 28.0;
const LEVEL: f32 = 112.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct Node {
    pub index: usize,
    pub left: f32,
    pub top: f32,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Line {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}
#[derive(Default, Debug)]
pub(super) struct Layout {
    pub nodes: Vec<Node>,
    pub lines: Vec<Line>,
    pub width: f32,
    pub height: f32,
}

pub(super) fn layout(parents: &[Option<usize>]) -> Result<Layout, &'static str> {
    if parents.len() > 32 {
        return Err("Choose at most 32 agents for a team.");
    }
    let mut children = vec![Vec::new(); parents.len()];
    let mut roots = Vec::new();
    for (index, parent) in parents.iter().enumerate() {
        if let Some(parent) = *parent {
            if parent >= parents.len() || parent == index {
                return Err("Choose a valid reporting agent.");
            }
            children[parent].push(index);
        } else {
            roots.push(index);
        }
    }
    fn measure(
        index: usize,
        children: &[Vec<usize>],
        marks: &mut [u8],
        widths: &mut [f32],
    ) -> Result<f32, &'static str> {
        if marks[index] == 1 {
            return Err("Reporting lines cannot contain a cycle.");
        }
        if marks[index] == 2 {
            return Ok(widths[index]);
        }
        marks[index] = 1;
        let mut width = 0.0;
        for (position, child) in children[index].iter().enumerate() {
            if position > 0 {
                width += GAP;
            }
            width += measure(*child, children, marks, widths)?;
        }
        widths[index] = width.max(NODE_WIDTH);
        marks[index] = 2;
        Ok(widths[index])
    }
    let mut marks = vec![0; parents.len()];
    let mut widths = vec![0.0; parents.len()];
    // Validate every component, including a cycle disconnected from the lead.
    for index in 0..parents.len() {
        measure(index, &children, &mut marks, &mut widths)?;
    }
    fn place(
        index: usize,
        left: f32,
        depth: usize,
        children: &[Vec<usize>],
        widths: &[f32],
        result: &mut Layout,
    ) {
        let center = left + widths[index] / 2.0;
        let top = depth as f32 * LEVEL;
        result.nodes.push(Node {
            index,
            left: center - NODE_WIDTH / 2.0,
            top,
        });
        result.height = result.height.max(top + NODE_HEIGHT);
        let mut cursor = left;
        let mut centers = Vec::new();
        for child in &children[index] {
            centers.push(cursor + widths[*child] / 2.0);
            place(*child, cursor, depth + 1, children, widths, result);
            cursor += widths[*child] + GAP;
        }
        if !centers.is_empty() {
            let split = top + NODE_HEIGHT + (LEVEL - NODE_HEIGHT) / 2.0;
            result.lines.push(Line {
                left: center,
                top: top + NODE_HEIGHT,
                width: 1.0,
                height: split - top - NODE_HEIGHT,
            });
            let first = centers[0].min(center);
            let last = centers[centers.len() - 1].max(center);
            result.lines.push(Line {
                left: first,
                top: split,
                width: (last - first).max(1.0),
                height: 1.0,
            });
            for child_center in centers {
                result.lines.push(Line {
                    left: child_center,
                    top: split,
                    width: 1.0,
                    height: (depth + 1) as f32 * LEVEL - split,
                });
            }
        }
    }
    let mut result = Layout::default();
    let mut left = 0.0;
    for root in roots {
        place(root, left, 0, &children, &widths, &mut result);
        left += widths[root] + GAP;
    }
    result.width = if result.nodes.is_empty() {
        0.0
    } else {
        left - GAP
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branched_team_keeps_children_below_their_manager_without_overlap() {
        // Lead -> CMO, CTO, COO; CTO -> frontend, backend.
        let graph = layout(&[None, Some(0), Some(0), Some(0), Some(2), Some(2)]).unwrap();
        assert_eq!(graph.nodes.len(), 6);
        for parent_depth in [0.0, LEVEL, LEVEL * 2.0] {
            let mut row: Vec<_> = graph
                .nodes
                .iter()
                .filter(|n| n.top == parent_depth)
                .collect();
            row.sort_by(|a, b| a.left.partial_cmp(&b.left).unwrap());
            for pair in row.windows(2) {
                assert!(pair[0].left + NODE_WIDTH <= pair[1].left);
            }
        }
        let manager = graph.nodes.iter().find(|n| n.index == 2).unwrap();
        for child in graph.nodes.iter().filter(|n| n.index == 4 || n.index == 5) {
            assert!(child.top > manager.top);
            assert!(child.left >= 0.0 && child.left + NODE_WIDTH <= graph.width);
        }
        for line in &graph.lines {
            assert!(line.left.is_finite() && line.top.is_finite());
            assert!(line.width > 0.0 && line.height > 0.0);
            assert!(line.left >= 0.0 && line.left + line.width <= graph.width);
            assert!(line.top >= 0.0 && line.top + line.height <= graph.height);
        }
    }

    #[test]
    fn cycle_in_a_disconnected_branch_is_rejected() {
        assert!(layout(&[None, Some(2), Some(1)]).is_err());
    }

    #[test]
    fn invalid_or_self_reporting_is_rejected() {
        assert!(layout(&[Some(0)]).is_err());
        assert!(layout(&[None, Some(9)]).is_err());
    }

    #[test]
    fn empty_and_single_agent_graphs_have_finite_bounds() {
        assert_eq!(layout(&[]).unwrap().width, 0.0);
        let graph = layout(&[None]).unwrap();
        assert_eq!(graph.width, NODE_WIDTH);
        assert_eq!(graph.height, NODE_HEIGHT);
        assert!(graph.lines.is_empty());
    }
}
