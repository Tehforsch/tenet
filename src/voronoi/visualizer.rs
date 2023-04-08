use super::constructor::SearchData;
use super::delaunay::dimension::DDimension;
use super::delaunay::Delaunay;
use super::delaunay::PointKind;
use super::delaunay::TetraIndex;
use super::primitives::triangle::TriangleData;
use super::primitives::Point2d;
use super::primitives::Point3d;
use super::Cell;
use super::Triangulation;
use super::TwoD;
use crate::hash_map::HashMap;

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Statement {
    statement: String,
    is_new_item: bool,
}

impl From<String> for Statement {
    fn from(statement: String) -> Self {
        Self {
            statement,
            is_new_item: true,
        }
    }
}

impl From<Statement> for String {
    fn from(s: Statement) -> Self {
        s.statement
    }
}

pub type Name = String;

#[derive(Default)]
pub struct Visualizer {
    statement_names: HashMap<Statement, Name>,
    statements: Vec<Statement>,
}

impl Visualizer {
    fn get_new_statement_name(&mut self) -> Name {
        format!("A_{}", self.statement_names.len())
    }

    fn add_statement(&mut self, statement: Statement) -> Name {
        let new_name = self.get_new_statement_name();
        if !self.statement_names.contains_key(&statement) {
            self.statement_names.insert(statement.clone(), new_name);
            self.statements.push(statement.clone());
        }
        self.statement_names[&statement].clone()
    }

    pub fn add(&mut self, p: &impl Visualizable) -> Vec<Name> {
        p.get_statements(self)
            .into_iter()
            .map(|statement| self.add_statement(statement))
            .collect()
    }

    fn dump(&self) {
        // The second list is to make sure we iterate in the correct order. Hacky but who cares
        let statements: Vec<_> = self
            .statements
            .iter()
            .map(|statement| {
                if statement.is_new_item {
                    format!(
                        "\"{} = {}\"",
                        &self.statement_names[statement], &statement.statement
                    )
                } else {
                    format!("\"{}\"", &statement.statement)
                }
            })
            .collect();
        println!("Execute({{ {} }})", statements.join(", "));
    }
}

impl Drop for Visualizer {
    fn drop(&mut self) {
        self.dump();
    }
}

pub trait Visualizable {
    fn get_statements(&self, vis: &mut Visualizer) -> Vec<Statement>;
}

impl Visualizable for TriangleData<Point2d> {
    fn get_statements(&self, visualizer: &mut Visualizer) -> Vec<Statement> {
        let points = [self.p1, self.p2, self.p3];
        let point_names: Vec<_> = points
            .into_iter()
            .map(|p| visualizer.add(&p)[0].clone())
            .collect();
        vec![format!(
            "Polygon({}, {}, {})",
            point_names[0], point_names[1], point_names[2]
        )
        .into()]
    }
}

impl Visualizable for Vec<Point3d> {
    fn get_statements(&self, visualizer: &mut Visualizer) -> Vec<Statement> {
        self.iter()
            .map(|p| p.get_statements(visualizer)[0].statement.clone().into())
            .collect()
    }
}

impl Visualizable for Vec<Point2d> {
    fn get_statements(&self, visualizer: &mut Visualizer) -> Vec<Statement> {
        self.iter()
            .map(|p| p.get_statements(visualizer)[0].statement.clone().into())
            .collect()
    }
}

impl Visualizable for super::primitives::tetrahedron::TetrahedronData {
    fn get_statements(&self, visualizer: &mut Visualizer) -> Vec<Statement> {
        use super::utils::periodic_windows_3;
        let points = [self.p1, self.p2, self.p3, self.p4];
        let point_names: Vec<_> = points
            .into_iter()
            .map(|p| visualizer.add(&p)[0].clone())
            .collect();
        periodic_windows_3(&point_names)
            .map(|(p1, p2, p3)| format!("Polygon({}, {}, {})", p1, p2, p3).into())
            .collect()
    }
}

impl<D> Visualizable for Triangulation<D>
where
    D: DDimension,
    Triangulation<D>: Delaunay<D>,
    <D as DDimension>::TetraData: Visualizable,
{
    fn get_statements(&self, visualizer: &mut Visualizer) -> Vec<Statement> {
        self.points.iter().for_each(|(index, point)| {
            let color = match self.point_kinds[&index] {
                PointKind::Inner => (1.0, 0.0, 0.0),
                PointKind::Outer => (0.0, 1.0, 0.0),
                PointKind::Halo(_) => (0.0, 0.0, 1.0),
            };
            visualizer.add(&Color { x: *point, color });
        });
        self.tetras
            .iter()
            .flat_map(|(_, tetra)| self.get_tetra_data(tetra).get_statements(visualizer))
            .collect()
    }
}

impl<D> Visualizable for (&Triangulation<D>, TetraIndex)
where
    D: DDimension,
    Triangulation<D>: Delaunay<D>,
    <D as DDimension>::TetraData: Visualizable,
{
    fn get_statements(&self, visualizer: &mut Visualizer) -> Vec<Statement> {
        self.0
            .get_tetra_data(&self.0.tetras[self.1])
            .get_statements(visualizer)
    }
}

impl Visualizable for Cell<TwoD> {
    fn get_statements(&self, visualizer: &mut Visualizer) -> Vec<Statement> {
        let points: Vec<_> = self
            .points
            .iter()
            .map(|p| p.get_statements(visualizer)[0].statement.clone())
            .collect();
        vec![format!("Polygon({})", points.join(",")).into()]
    }
}

impl Visualizable for Point3d {
    fn get_statements(&self, _visualizer: &mut Visualizer) -> Vec<Statement> {
        vec![format!("({}, {}, {})", self.x, self.y, self.z).into()]
    }
}

impl Visualizable for Point2d {
    fn get_statements(&self, _visualizer: &mut Visualizer) -> Vec<Statement> {
        vec![format!("({}, {})", self.x, self.y).into()]
    }
}

impl<D> Visualizable for SearchData<D>
where
    D: DDimension,
{
    fn get_statements(&self, visualizer: &mut Visualizer) -> Vec<Statement> {
        let s: String = self.point.get_statements(visualizer)[0].clone().into();
        vec![format!("Circle({}, {})", &s, self.radius).into()]
    }
}

pub struct Color<T> {
    pub x: T,
    pub color: (f64, f64, f64),
}

impl<T: Visualizable> Visualizable for Color<T> {
    fn get_statements(&self, visualizer: &mut Visualizer) -> Vec<Statement> {
        let statements = self.x.get_statements(visualizer);
        statements
            .into_iter()
            .map(|statement| {
                let name = visualizer.add_statement(statement);
                Statement {
                    statement: format!(
                        "SetDynamicColor({}, {}, {}, {}, 0.7)",
                        name, self.color.0, self.color.1, self.color.2
                    ),
                    is_new_item: false,
                }
            })
            .collect()
    }
}

#[macro_export]
macro_rules! vis {
    ( $( $x:expr ),* ) => {
        {
            let mut temp_vis = $crate::voronoi::visualizer::Visualizer::default();
            $(
                temp_vis.add($x);
            )*
        }
    };
}

#[macro_export]
macro_rules! highlight_red {
    ( $x:expr) => {{
        &$crate::voronoi::visualizer::Color {
            x: Box::new($x.clone()),
            color: (1.0, 0.0, 0.0),
        }
    }};
}

#[macro_export]
macro_rules! highlight_blue {
    ( $x:expr) => {{
        &$crate::voronoi::visualizer::Color {
            x: Box::new($x.clone()),
            color: (0.0, 0.0, 1.0),
        }
    }};
}

#[macro_export]
macro_rules! highlight_green {
    ( $x:expr) => {{
        &$crate::voronoi::visualizer::Color {
            x: Box::new($x.clone()),
            color: (0.0, 1.0, 0.0),
        }
    }};
}
