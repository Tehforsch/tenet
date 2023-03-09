use super::DelaunayTriangulation;
use crate::voronoi::delaunay::FlipCheckData;
use crate::voronoi::tetra::ConnectionData;
use crate::voronoi::tetra::Tetra;
use crate::voronoi::tetra::TetraData;
use crate::voronoi::tetra::TetraFace;
use crate::voronoi::Face;
use crate::voronoi::FaceIndex;
use crate::voronoi::PointIndex;
use crate::voronoi::TetraIndex;

impl DelaunayTriangulation {
    pub fn get_tetra_data(&self, tetra: &Tetra) -> TetraData {
        TetraData {
            p1: self.points[tetra.p1],
            p2: self.points[tetra.p2],
            p3: self.points[tetra.p3],
            #[cfg(feature = "3d")]
            p4: self.points[tetra.p4],
        }
    }

    pub(super) fn make_positively_oriented_tetra(&mut self, tetra: Tetra) -> Tetra {
        let tetra_data = TetraData {
            p1: self.points[tetra.p1],
            p2: self.points[tetra.p2],
            p3: self.points[tetra.p3],
        };
        debug_assert!(self.faces[tetra.f1.face].contains_point(tetra.p2));
        debug_assert!(self.faces[tetra.f1.face].contains_point(tetra.p3));
        debug_assert!(self.faces[tetra.f2.face].contains_point(tetra.p1));
        debug_assert!(self.faces[tetra.f2.face].contains_point(tetra.p3));
        debug_assert!(self.faces[tetra.f3.face].contains_point(tetra.p1));
        debug_assert!(self.faces[tetra.f3.face].contains_point(tetra.p2));
        if tetra_data.is_positively_oriented() {
            tetra
        } else {
            Tetra {
                p1: tetra.p2,
                p2: tetra.p1,
                p3: tetra.p3,
                f1: tetra.f2,
                f2: tetra.f1,
                f3: tetra.f3,
            }
        }
    }

    fn insert_split_tetra(
        &mut self,
        p_a: PointIndex,
        p_b: PointIndex,
        p: PointIndex,
        f_a: FaceIndex,
        f_b: FaceIndex,
        old_face: TetraFace,
    ) -> TetraIndex {
        // Leave opposing data of the newly created faces
        // uninitialized for now, since we do not know the indices of
        // the other tetras before we have inserted them.
        self.insert_positively_oriented_tetra(Tetra {
            p1: p_a,
            p2: p_b,
            p3: p,
            f1: TetraFace {
                face: f_a,
                opposing: None,
            },
            f2: TetraFace {
                face: f_b,
                opposing: None,
            },
            f3: old_face,
        })
    }

    pub(super) fn split(&mut self, old_tetra_index: TetraIndex, point: PointIndex) {
        let old_tetra = self.tetras.remove(old_tetra_index).unwrap();
        let f1 = self.faces.insert(Face {
            p1: point,
            p2: old_tetra.p1,
        });
        let f2 = self.faces.insert(Face {
            p1: point,
            p2: old_tetra.p2,
        });
        let f3 = self.faces.insert(Face {
            p1: point,
            p2: old_tetra.p3,
        });
        let t1 = self.insert_split_tetra(old_tetra.p2, old_tetra.p3, point, f3, f2, old_tetra.f1);
        let t2 = self.insert_split_tetra(old_tetra.p3, old_tetra.p1, point, f1, f3, old_tetra.f2);
        let t3 = self.insert_split_tetra(old_tetra.p1, old_tetra.p2, point, f2, f1, old_tetra.f3);
        self.set_opposing_in_new_tetra(t1, f3, t2, old_tetra.p1);
        self.set_opposing_in_new_tetra(t1, f2, t3, old_tetra.p1);
        self.set_opposing_in_new_tetra(t2, f3, t1, old_tetra.p2);
        self.set_opposing_in_new_tetra(t2, f1, t3, old_tetra.p2);
        self.set_opposing_in_new_tetra(t3, f1, t2, old_tetra.p3);
        self.set_opposing_in_new_tetra(t3, f2, t1, old_tetra.p3);
        for (tetra, face) in [(t1, old_tetra.f1), (t2, old_tetra.f2), (t3, old_tetra.f3)] {
            self.to_check.push(FlipCheckData {
                tetra,
                face: face.face,
            });
        }
    }

    pub(super) fn flip(&mut self, check: FlipCheckData) {
        let old_tetra = self.tetras.remove(check.tetra).unwrap();
        let old_face = self.faces.remove(check.face).unwrap();
        // I am not sure whether unwrapping here is correct -
        // can a boundary face require a flip? what does that even mean?
        let opposing = old_tetra.find_face(check.face).opposing.clone().unwrap();
        let opposing_old_tetra = self.tetras.remove(opposing.tetra).unwrap();
        let opposing_point = opposing.point;
        let check_point = old_tetra.find_point_opposite(check.face);
        let new_face = self.faces.insert(Face {
            p1: check_point,
            p2: opposing_point,
        });

        let f1_a = opposing_old_tetra.find_face_opposite(old_face.p2).clone();
        let f1_b = old_tetra.find_face_opposite(old_face.p2).clone();
        let f2_a = opposing_old_tetra.find_face_opposite(old_face.p1).clone();
        let f2_b = old_tetra.find_face_opposite(old_face.p1).clone();

        let t1 = self.insert_positively_oriented_tetra(Tetra {
            p1: old_face.p1,
            p2: check_point,
            p3: opposing_point,
            // Leave uninitialized for now
            f1: TetraFace {
                face: new_face,
                opposing: None,
            },
            f2: f1_a,
            f3: f1_b,
        });
        let t2 = self.insert_positively_oriented_tetra(Tetra {
            p1: old_face.p2,
            p2: check_point,
            p3: opposing_point,
            // Leave uninitialized for now
            f1: TetraFace {
                face: new_face,
                opposing: None,
            },
            f2: f2_a,
            f3: f2_b,
        });
        // Set previously uninitialized opposing data, now that we know the tetra indices
        self.tetras[t1].find_face_mut(new_face).opposing = Some(ConnectionData {
            tetra: t2,
            point: old_face.p2,
        });
        self.tetras[t2].find_face_mut(new_face).opposing = Some(ConnectionData {
            tetra: t1,
            point: old_face.p1,
        });
        // Now that we have flipped this edge, we have to check the remaining edges
        // in the opposing tetra as well
        self.to_check.push(FlipCheckData {
            tetra: t1,
            face: f1_a.face,
        });
        self.to_check.push(FlipCheckData {
            tetra: t2,
            face: f2_a.face,
        });
    }

    pub fn insert_basic_tetra(&mut self, tetra: TetraData) {
        let p1 = self.points.insert(tetra.p1);
        let p2 = self.points.insert(tetra.p2);
        let p3 = self.points.insert(tetra.p3);
        let f1 = self.faces.insert(Face { p1: p2, p2: p3 });
        let f2 = self.faces.insert(Face { p1: p3, p2: p1 });
        let f3 = self.faces.insert(Face { p1: p1, p2: p2 });
        self.insert_positively_oriented_tetra(Tetra {
            p1,
            p2,
            p3,
            f1: TetraFace {
                face: f1,
                opposing: None,
            },
            f2: TetraFace {
                face: f2,
                opposing: None,
            },
            f3: TetraFace {
                face: f3,
                opposing: None,
            },
        });
    }
}