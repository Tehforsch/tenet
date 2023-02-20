mod delaunay;
mod face;
mod indexed_arena;
mod tetra;

use std::iter;

use bevy::prelude::Resource;
use bevy::utils::StableHashMap;
pub use delaunay::DelaunayTriangulation;
use derive_more::From;
use derive_more::Into;
use generational_arena::Index;
use ordered_float::OrderedFloat;

use self::face::Face;
use self::indexed_arena::IndexedArena;
use self::tetra::sign;
use self::tetra::Tetra;

#[derive(Debug, Clone, Copy, From, Into, PartialEq, Eq)]
pub struct TetraIndex(Index);
#[derive(Debug, Clone, Copy, From, Into, PartialEq, Eq)]
pub struct FaceIndex(Index);
#[derive(Debug, Clone, Copy, From, Into, PartialEq, Eq, Hash)]
pub struct PointIndex(Index);

pub type CellIndex = usize;

#[cfg(feature = "2d")]
pub type Point = glam::DVec2;
#[cfg(feature = "3d")]
pub type Point = glam::DVec3;

type TetraList = IndexedArena<TetraIndex, Tetra>;
type FaceList = IndexedArena<FaceIndex, Face>;
type PointList = IndexedArena<PointIndex, Point>;

#[derive(Resource)]
pub struct VoronoiGrid {
    pub cells: Vec<Cell>,
}

pub struct Cell {
    pub delaunay_point: PointIndex,
    pub points: Vec<Point>,
    pub connected_cells: Vec<CellIndex>,
    pub is_boundary: bool,
}

/// Like slice.windows but including (t.last(), t.first()) as a last item.
/// Returns an empty iterator on a slice with one or zero elements.
fn periodic_windows<T>(v: &[T]) -> impl Iterator<Item = (&T, &T)> {
    v.iter()
        .zip(v[1..].iter().chain(iter::once(&v[0])))
        .filter(|_| v.len() > 1)
}

impl Cell {
    pub fn point_windows(&self) -> impl Iterator<Item = (&Point, &Point)> {
        periodic_windows(&self.points)
    }

    pub fn contains(&self, point: Point) -> bool {
        let has_negative = self
            .point_windows()
            .map(|(p1, p2)| sign(point, *p1, *p2))
            .any(|s| s < 0.0);
        let has_positive = self
            .point_windows()
            .map(|(p1, p2)| sign(point, *p1, *p2))
            .any(|s| s > 0.0);

        !(has_negative && has_positive)
    }
}

impl From<DelaunayTriangulation> for VoronoiGrid {
    fn from(t: DelaunayTriangulation) -> Self {
        let mut map: StableHashMap<PointIndex, CellIndex> = StableHashMap::default();
        let point_to_tetra_map = point_to_tetra_map(&t);
        let mut cells = vec![];
        for (i, (point_index, _)) in t.points.iter().enumerate() {
            map.insert(point_index, i);
        }
        for (point_index, _) in t.points.iter() {
            let mut points = vec![];
            let mut connected_cells = vec![];
            let tetras = &point_to_tetra_map[&point_index];
            for tetra in tetras.iter() {
                points.push(
                    t.get_tetra_data(&t.tetras[*tetra])
                        .get_center_of_circumcircle(),
                );
            }
            let mut is_boundary = false;
            for (t1, t2) in periodic_windows(tetras) {
                let common_face = t.tetras[*t1].get_common_face_with(&t.tetras[*t2]);
                if let Some(common_face) = common_face {
                    let other_point = t.faces[common_face].get_other_point(point_index);
                    connected_cells.push(map[&other_point]);
                } else {
                    is_boundary = true;
                }
            }
            cells.push(Cell {
                delaunay_point: point_index,
                points,
                connected_cells,
                is_boundary,
            });
        }
        VoronoiGrid { cells }
    }
}

fn point_to_tetra_map(
    triangulation: &DelaunayTriangulation,
) -> StableHashMap<PointIndex, Vec<TetraIndex>> {
    let mut map: StableHashMap<_, _> = triangulation
        .points
        .iter()
        .map(|(i, _)| (i, vec![]))
        .collect();
    for (tetra_index, tetra) in triangulation.tetras.iter() {
        map.get_mut(&tetra.p1).unwrap().push(tetra_index);
        map.get_mut(&tetra.p2).unwrap().push(tetra_index);
        map.get_mut(&tetra.p3).unwrap().push(tetra_index);
    }
    for (point_index, tetras) in map.iter_mut() {
        let point = triangulation.points[*point_index];
        tetras.sort_by_key(|t| {
            let p = triangulation
                .get_tetra_data(&triangulation.tetras[*t])
                .get_center_of_circumcircle();
            let vec = p - point;
            OrderedFloat(vec.x.atan2(vec.y))
        });
    }
    map
}

#[cfg(test)]
mod tests {
    use ordered_float::OrderedFloat;

    use super::delaunay::tests::perform_check_on_each_level_of_construction;
    use super::Cell;
    use super::Point;
    use super::VoronoiGrid;

    #[test]
    fn voronoi_property() {
        perform_check_on_each_level_of_construction(|triangulation, _| {
            let grid = VoronoiGrid::from(triangulation.clone());
            for (i, j) in (0..10).zip(0..10) {
                let lookup_point = Point::new(0.1 * i as f64, 0.1 * j as f64);
                let containing_cell = get_containing_voronoi_cell(&grid, lookup_point);
                let closest_cell = grid
                    .cells
                    .iter()
                    .min_by_key(|cell| {
                        let p = triangulation.points[cell.delaunay_point];
                        OrderedFloat(p.distance_squared(lookup_point))
                    })
                    .unwrap();
                if let Some(containing_cell) = containing_cell {
                    assert_eq!(containing_cell.delaunay_point, closest_cell.delaunay_point);
                }
            }
        });
    }

    fn get_containing_voronoi_cell(grid: &VoronoiGrid, point: Point) -> Option<&Cell> {
        grid.cells.iter().find(|cell| cell.contains(point))
    }
}
