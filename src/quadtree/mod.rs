mod config;
mod index;
mod node_index;
mod visualization;

pub use config::QuadTreeConfig;
pub use index::QuadTreeIndex;
pub use visualization::QuadTreeVisualizationPlugin;

use crate::domain::extent::Extent;
use crate::units::VecLength;

pub const MAX_DEPTH: usize = 32;
pub const NUM_DIMENSIONS: usize = 2;
pub const NUM_SUBDIVISIONS: usize = 2usize.pow(NUM_DIMENSIONS as u32);

pub trait QuadTreeLeafData: Clone {
    fn pos(&self) -> &VecLength;
}

pub trait QuadTreeNodeData<L>: Default {
    fn update_with(&mut self, leaf: &L);
}

type Tree<N, L> = Box<[QuadTree<N, L>; 4]>;
type Leaf<L> = Vec<L>;

#[derive(Debug)]
pub enum Node<N, L> {
    Tree(Tree<N, L>),
    Leaf(Leaf<L>),
}

impl<N, L> Node<N, L> {
    fn make_node(&mut self, children: Tree<N, L>) -> Leaf<L> {
        let value = std::mem::replace(self, Node::Tree(children));
        if let Self::Leaf(leaf) = value {
            leaf
        } else {
            panic!("make_node called on Node value")
        }
    }
}

#[derive(Debug)]
pub struct QuadTree<N, L> {
    pub node: Node<N, L>,
    pub data: N,
    pub extent: Extent,
}

impl<N: QuadTreeNodeData<L>, L: QuadTreeLeafData> QuadTree<N, L> {
    pub fn new(config: &QuadTreeConfig, particles: Vec<L>, extent: &Extent) -> Self {
        let mut tree = Self::make_empty_leaf_from_extent(extent.clone());
        tree.subdivide_to_depth(config, config.min_depth);
        for particle in particles.iter() {
            tree.insert_new(config, particle.clone(), 0);
        }
        tree
    }

    fn subdivide_to_depth(&mut self, config: &QuadTreeConfig, depth: usize) {
        if depth > 0 {
            self.subdivide(config, depth);
            if let Node::Tree(ref mut children) = self.node {
                for child in children.iter_mut() {
                    child.subdivide_to_depth(config, depth - 1);
                }
            } else {
                unreachable!()
            }
        }
    }

    fn insert_new(&mut self, config: &QuadTreeConfig, leaf_data: L, depth: usize) {
        self.data.update_with(&leaf_data);
        self.insert(config, leaf_data, depth)
    }

    fn insert(&mut self, config: &QuadTreeConfig, leaf_data: L, depth: usize) {
        if let Node::Leaf(ref mut leaf) = self.node {
            if depth < config.max_depth && leaf.len() > config.max_num_particles_per_leaf {
                self.subdivide(config, depth);
            } else {
                leaf.push(leaf_data);
                return;
            }
        }
        if let Node::Tree(ref mut children) = self.node {
            let quadrant = &mut children[self.extent.get_quadrant_index(leaf_data.pos())];
            quadrant.insert_new(config, leaf_data, depth + 1);
        }
    }

    fn subdivide(&mut self, config: &QuadTreeConfig, depth: usize) {
        debug_assert!(matches!(self.node, Node::Leaf(_)));
        let quadrants = self.extent.get_quadrants();
        let children = Box::new(quadrants.map(Self::make_empty_leaf_from_extent));
        let particles = self.node.make_node(children);
        for particle in particles.into_iter() {
            self.insert(config, particle, depth);
        }
    }

    pub fn make_empty_leaf_from_extent(extent: Extent) -> Self {
        Self {
            node: Node::Leaf(vec![]),
            data: N::default(),
            extent,
        }
    }

    pub fn depth_first_map_leaf<'a>(&'a self, closure: &mut impl FnMut(&'a Extent, &'a [L])) {
        match self.node {
            Node::Tree(ref node) => {
                for child in node.iter() {
                    child.depth_first_map_leaf(closure);
                }
            }
            Node::Leaf(ref leaf) => {
                closure(&self.extent, leaf);
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use bevy::prelude::Entity;

    use super::*;
    use crate::physics::gravity::LeafData;
    use crate::units::DVec2Length;
    use crate::units::Length;
    use crate::units::Mass;

    impl QuadTreeLeafData for VecLength {
        fn pos(&self) -> &VecLength {
            self
        }
    }
    impl<T> QuadTreeNodeData<T> for () {
        fn update_with(&mut self, _: &T) {}
    }

    #[test]
    fn no_infinite_recursion_in_tree_construction_with_close_particles() {
        let positions = [
            LeafData {
                entity: Entity::from_raw(0),
                pos: DVec2Length::meters(1.0, 1.0),
                mass: Mass::zero(),
            },
            LeafData {
                entity: Entity::from_raw(0),
                pos: DVec2Length::meters(1.0, 1.0),
                mass: Mass::zero(),
            },
            LeafData {
                entity: Entity::from_raw(0),
                pos: DVec2Length::meters(2.0, 2.0),
                mass: Mass::zero(),
            },
        ];
        let config = QuadTreeConfig {
            max_depth: 10,
            ..Default::default()
        };
        let extent =
            Extent::from_positions(positions.iter().map(|particle| &particle.pos)).unwrap();
        QuadTree::<(), LeafData>::new(&config, positions.into_iter().collect(), &extent);
    }

    pub fn get_min_depth_quadtree<N: QuadTreeNodeData<L>, L: QuadTreeLeafData>(
        min_depth: usize,
    ) -> QuadTree<N, L> {
        let positions = [];
        let config = QuadTreeConfig {
            min_depth,
            max_depth: 10,
            ..Default::default()
        };
        let extent = Extent::new(
            Length::meters(0.0),
            Length::meters(1.0),
            Length::meters(0.0),
            Length::meters(1.0),
        );
        QuadTree::new(&config, positions.into_iter().collect(), &extent)
    }

    #[test]
    fn min_depth_works() {
        for min_depth in 0..5 {
            let tree: QuadTree<(), VecLength> = get_min_depth_quadtree(min_depth);
            let mut num_nodes = 0;
            let mut count = |_, _| {
                num_nodes += 1;
            };
            tree.depth_first_map_leaf(&mut count);
            assert_eq!(num_nodes, 4usize.pow(min_depth as u32));
        }
    }
}
