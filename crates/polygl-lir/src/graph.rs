use crate::SymbolId;

pub(crate) struct DependencyGraph {
    edges: Vec<Vec<SymbolId>>,
}

impl DependencyGraph {
    pub(crate) fn new(node_count: usize) -> Self {
        Self {
            edges: vec![Vec::new(); node_count],
        }
    }

    pub(crate) fn set_dependencies(
        &mut self,
        symbol: SymbolId,
        dependencies: impl IntoIterator<Item = SymbolId>,
    ) {
        let edges = &mut self.edges[symbol.index()];
        edges.extend(dependencies);
        edges.sort_unstable();
        edges.dedup();
    }

    #[must_use]
    pub(crate) fn dependencies(&self, symbol: SymbolId) -> &[SymbolId] {
        &self.edges[symbol.index()]
    }

    /// Returns SCCs with dependencies before their dependents. Both component
    /// and member order are stable for a fixed symbol table.
    pub(crate) fn strongly_connected_components(&self) -> Vec<Vec<SymbolId>> {
        let mut visited = vec![false; self.edges.len()];
        let mut finished = Vec::with_capacity(self.edges.len());
        for root in 0..self.edges.len() {
            if visited[root] {
                continue;
            }
            visited[root] = true;
            let mut stack = vec![(root, 0)];
            while let Some((node, edge_index)) = stack.last_mut() {
                if let Some(dependency) = self.edges[*node].get(*edge_index) {
                    *edge_index += 1;
                    let dependency = dependency.index();
                    if !visited[dependency] {
                        visited[dependency] = true;
                        stack.push((dependency, 0));
                    }
                } else {
                    finished.push(SymbolId::from_index(*node));
                    stack.pop();
                }
            }
        }

        let mut reverse = vec![Vec::new(); self.edges.len()];
        for (source, dependencies) in self.edges.iter().enumerate() {
            for dependency in dependencies {
                reverse[dependency.index()].push(SymbolId::from_index(source));
            }
        }
        for edges in &mut reverse {
            edges.sort_unstable();
        }

        visited.fill(false);
        let mut components = Vec::new();
        for root in finished.into_iter().rev() {
            if visited[root.index()] {
                continue;
            }
            visited[root.index()] = true;
            let mut pending = vec![root];
            let mut component = Vec::new();
            while let Some(node) = pending.pop() {
                component.push(node);
                for dependent in reverse[node.index()].iter().rev() {
                    if !visited[dependent.index()] {
                        visited[dependent.index()] = true;
                        pending.push(*dependent);
                    }
                }
            }
            component.sort_unstable();
            components.push(component);
        }
        components.reverse();
        components
    }
}

#[cfg(test)]
mod tests {
    use crate::SymbolId;

    use super::DependencyGraph;

    #[test]
    fn returns_cycles_once_and_before_their_dependents() {
        let ids = (0..4).map(SymbolId::from_index).collect::<Vec<_>>();
        let mut graph = DependencyGraph::new(ids.len());
        graph.set_dependencies(ids[0], [ids[1]]);
        graph.set_dependencies(ids[1], [ids[2]]);
        graph.set_dependencies(ids[2], [ids[1]]);
        graph.set_dependencies(ids[3], [ids[0]]);

        assert_eq!(
            graph.strongly_connected_components(),
            vec![vec![ids[1], ids[2]], vec![ids[0]], vec![ids[3]]]
        );
    }
}
