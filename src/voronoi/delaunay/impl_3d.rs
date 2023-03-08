use super::DelaunayTriangulation;
use super::FlipCheckData;
use crate::voronoi::face::Face;
use crate::voronoi::face::FaceData;
use crate::voronoi::face::IntersectionType;
use crate::voronoi::tetra::ConnectionData;
use crate::voronoi::tetra::Tetra;
use crate::voronoi::tetra::TetraData;
use crate::voronoi::tetra::TetraFace;
use crate::voronoi::utils::periodic_windows_3;
use crate::voronoi::FaceIndex;
use crate::voronoi::PointIndex;
use crate::voronoi::TetraIndex;

impl DelaunayTriangulation {
    pub fn get_tetra_data(&self, tetra: &Tetra) -> TetraData {
        TetraData {
            p1: self.points[tetra.p1],
            p2: self.points[tetra.p2],
            p3: self.points[tetra.p3],
            p4: self.points[tetra.p4],
        }
    }

    pub fn get_face_data(&self, face: &Face) -> FaceData {
        FaceData {
            p1: self.points[face.p1],
            p2: self.points[face.p2],
            p3: self.points[face.p3],
        }
    }

    fn make_tetra(
        &mut self,
        p_a: PointIndex,
        p_b: PointIndex,
        p_c: PointIndex,
        p: PointIndex,
        f_a: FaceIndex,
        f_b: FaceIndex,
        f_c: FaceIndex,
        old_face: TetraFace,
    ) -> TetraIndex {
        // Leave opposing data of the newly created faces
        // uninitialized for now, since we do not know the indices of
        // the other tetras before we have inserted them.
        self.insert_positively_oriented_tetra(
            p_a,
            p_b,
            p_c,
            p,
            TetraFace {
                face: f_a,
                opposing: None,
            },
            TetraFace {
                face: f_b,
                opposing: None,
            },
            TetraFace {
                face: f_c,
                opposing: None,
            },
            old_face,
        )
    }

    fn insert_positively_oriented_tetra(
        &mut self,
        p1: PointIndex,
        p2: PointIndex,
        p3: PointIndex,
        p4: PointIndex,
        f1: TetraFace,
        f2: TetraFace,
        f3: TetraFace,
        f4: TetraFace,
    ) -> TetraIndex {
        let tetra_data = TetraData {
            p1: self.points[p1],
            p2: self.points[p2],
            p3: self.points[p3],
            p4: self.points[p4],
        };
        for (f, (pa, pb, pc)) in [
            (f1.face, (p2, p3, p4)),
            (f2.face, (p1, p3, p4)),
            (f3.face, (p1, p2, p4)),
            (f4.face, (p1, p2, p3)),
        ] {
            debug_assert!(self.faces[f].contains_point(pa));
            debug_assert!(self.faces[f].contains_point(pb));
            debug_assert!(self.faces[f].contains_point(pc));
        }
        let tetra = if tetra_data.is_positively_oriented() {
            Tetra {
                p1,
                p2,
                p3,
                p4,
                f1,
                f2,
                f3,
                f4,
            }
        } else {
            Tetra {
                p1: p2,
                p2: p1,
                p3,
                p4,
                f1: f2,
                f2: f1,
                f3,
                f4,
            }
        };
        debug_assert!(self.get_tetra_data(&tetra).is_positively_oriented());
        self.tetras.insert(tetra)
    }

    pub(super) fn split(&mut self, old_tetra_index: TetraIndex, point: PointIndex) {
        let old_tetra = self.tetras.remove(old_tetra_index).unwrap();
        let f1 = self.faces.insert(Face {
            p1: point,
            p2: old_tetra.p1,
            p3: old_tetra.p2,
        });
        let f2 = self.faces.insert(Face {
            p1: point,
            p2: old_tetra.p1,
            p3: old_tetra.p3,
        });
        let f3 = self.faces.insert(Face {
            p1: point,
            p2: old_tetra.p1,
            p3: old_tetra.p4,
        });
        let f4 = self.faces.insert(Face {
            p1: point,
            p2: old_tetra.p2,
            p3: old_tetra.p3,
        });
        let f5 = self.faces.insert(Face {
            p1: point,
            p2: old_tetra.p2,
            p3: old_tetra.p4,
        });
        let f6 = self.faces.insert(Face {
            p1: point,
            p2: old_tetra.p3,
            p3: old_tetra.p4,
        });
        let t1 = self.make_tetra(
            old_tetra.p2,
            old_tetra.p3,
            old_tetra.p4,
            point,
            f6,
            f5,
            f4,
            old_tetra.f1,
        );
        let t2 = self.make_tetra(
            old_tetra.p1,
            old_tetra.p3,
            old_tetra.p4,
            point,
            f6,
            f3,
            f2,
            old_tetra.f2,
        );
        let t3 = self.make_tetra(
            old_tetra.p1,
            old_tetra.p2,
            old_tetra.p4,
            point,
            f5,
            f3,
            f1,
            old_tetra.f3,
        );
        let t4 = self.make_tetra(
            old_tetra.p1,
            old_tetra.p2,
            old_tetra.p3,
            point,
            f4,
            f2,
            f1,
            old_tetra.f4,
        );
        self.set_opposing_in_new_tetra(t1, f6, t2, old_tetra.p1);
        self.set_opposing_in_new_tetra(t1, f5, t3, old_tetra.p1);
        self.set_opposing_in_new_tetra(t1, f4, t4, old_tetra.p1);

        self.set_opposing_in_new_tetra(t2, f6, t1, old_tetra.p2);
        self.set_opposing_in_new_tetra(t2, f3, t3, old_tetra.p2);
        self.set_opposing_in_new_tetra(t2, f2, t4, old_tetra.p2);

        self.set_opposing_in_new_tetra(t3, f5, t1, old_tetra.p3);
        self.set_opposing_in_new_tetra(t3, f3, t2, old_tetra.p3);
        self.set_opposing_in_new_tetra(t3, f1, t4, old_tetra.p3);

        self.set_opposing_in_new_tetra(t4, f4, t1, old_tetra.p4);
        self.set_opposing_in_new_tetra(t4, f2, t2, old_tetra.p4);
        self.set_opposing_in_new_tetra(t4, f1, t3, old_tetra.p4);

        self.set_opposing_in_existing_tetra(old_tetra_index, old_tetra.f1, t1, point);
        self.set_opposing_in_existing_tetra(old_tetra_index, old_tetra.f2, t2, point);
        self.set_opposing_in_existing_tetra(old_tetra_index, old_tetra.f3, t3, point);
        self.set_opposing_in_existing_tetra(old_tetra_index, old_tetra.f4, t4, point);

        for (tetra, face) in [
            (t1, old_tetra.f1),
            (t2, old_tetra.f2),
            (t3, old_tetra.f3),
            (t4, old_tetra.f4),
        ] {
            self.to_check.push(FlipCheckData {
                tetra,
                face: face.face,
            });
        }
    }

    pub(super) fn flip(&mut self, check: FlipCheckData) {
        // Two tetrahedra are flagged for flipping. There are three possible cases here, depending on the
        // intersection of the shared face (triangle) and the line between the two points opposite of the shared face.
        // 1. If the intersection point lies inside the triangle, we do a 2-to-3-flip, in which the two tetrahedra are replaced by three
        // 2. If the intersection point lies outside one of the edges, we take into account the neighbouring tetrahedron
        //    along that edge and do a 3-to-2 flip in which the three tetrahedra are converted to two.
        // 3. If the intersection point lies outside two edges, the flip can be skipped. This seems like magic
        //    but it can be shown that flipping the remaining violating edges will restore delaunayhood.
        // For more information see Springel (2009), doi:10.1111/j.1365-2966.2009.15715.x
        let t1 = &self.tetras[check.tetra];
        let shared_face = &self.faces[check.face];
        let opposing = t1.find_face(check.face).opposing.clone().unwrap();
        let t2 = &self.tetras[opposing.tetra];
        // Obtain the two points opposite of the shared face
        let p1 = t1.find_point_opposite(check.face);
        let p2 = t2.find_point_opposite(check.face);
        let intersection_type = self
            .get_face_data(shared_face)
            .get_line_intersection_type(self.points[p1], self.points[p2]);
        match intersection_type {
            IntersectionType::Inside => {
                self.two_to_three_flip(check.tetra, opposing.tetra, p1, p2, check.face);
            }
            IntersectionType::OutsideOneEdge(edge) => {
                todo!()
                // self.three_to_two_flip(check.tetra, opposing.tetra, check.face);
            }
            IntersectionType::OutsideTwoEdges(_, _) => {}
        }
    }

    fn two_to_three_flip(
        &mut self,
        t1_index: TetraIndex,
        t2_index: TetraIndex,
        p1: PointIndex,
        p2: PointIndex,
        shared_face: FaceIndex,
    ) {
        let t1 = self.tetras.remove(t1_index).unwrap();
        let t2 = self.tetras.remove(t2_index).unwrap();
        let shared_face = self.faces.remove(shared_face).unwrap();
        let points = [shared_face.p1, shared_face.p2, shared_face.p3];
        let new_faces: Vec<_> = points
            .into_iter()
            .map(|p| {
                let new_face = self.faces.insert(Face { p1, p2, p3: p });
                (new_face, p)
            })
            .collect();
        let new_tetras: Vec<_> = periodic_windows_3(&new_faces)
            .map(|((fa, pa), (fb, pb), (_, other_point))| {
                let f1 = t1.find_face_opposite(*other_point).clone();
                let f2 = t2.find_face_opposite(*other_point).clone();
                let t = self.insert_positively_oriented_tetra(
                    p1,
                    p2,
                    *pa,
                    *pb,
                    f2,
                    f1,
                    // Leave opposing uninitialized for now
                    TetraFace {
                        face: *fb,
                        opposing: None,
                    },
                    TetraFace {
                        face: *fa,
                        opposing: None,
                    },
                );
                // Update the outdated connections in existing tetras
                self.set_opposing_in_existing_tetra(t1_index, f1, t, *other_point);
                self.set_opposing_in_existing_tetra(t2_index, f2, t, *other_point);
                (t, *fa, *fb, *pa, *pb)
            })
            .collect();
        // Set the connections between the newly created tetras
        for ((t_left, _, f_left, _, p_left), (t, _, _, _, _), (t_right, f_right, _, p_right, _)) in
            periodic_windows_3(&new_tetras)
        {
            for (tetra, face, point) in [(t_left, f_left, p_left), (t_right, f_right, p_right)] {
                self.tetras[*t].find_face_mut(*face).opposing = Some(ConnectionData {
                    tetra: *tetra,
                    point: *point,
                });
            }
        }
    }

    pub fn insert_basic_tetra(&mut self, tetra: TetraData) {
        debug_assert_eq!(self.tetras.len(), 0);
        let p1 = self.points.insert(tetra.p1);
        let p2 = self.points.insert(tetra.p2);
        let p3 = self.points.insert(tetra.p3);
        let p4 = self.points.insert(tetra.p4);
        let f1 = self.faces.insert(Face {
            p1: p2,
            p2: p3,
            p3: p4,
        });
        let f2 = self.faces.insert(Face {
            p1: p1,
            p2: p3,
            p3: p4,
        });
        let f3 = self.faces.insert(Face {
            p1: p1,
            p2: p2,
            p3: p4,
        });
        let f4 = self.faces.insert(Face {
            p1: p1,
            p2: p2,
            p3: p3,
        });
        self.insert_positively_oriented_tetra(
            p1,
            p2,
            p3,
            p4,
            TetraFace {
                face: f1,
                opposing: None,
            },
            TetraFace {
                face: f2,
                opposing: None,
            },
            TetraFace {
                face: f3,
                opposing: None,
            },
            TetraFace {
                face: f4,
                opposing: None,
            },
        );
    }
}
