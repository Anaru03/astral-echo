use render_core::*;

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn approx_mat(a: Mat4, b: Mat4) -> bool {
    a.to_cols_array()
        .iter()
        .zip(b.to_cols_array().iter())
        .all(|(x, y)| approx(*x, *y))
}

#[test]
fn identity_matrix() {
    let id = Mat4::IDENTITY;
    let v = Vec3::new(1.0, 2.0, 3.0);
    assert_eq!(id.transform_point3(v), v);
    assert!(approx_mat(id * id, id));
}

#[test]
fn trs_transforms() {
    let t = Transform::from_translation(Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(
        t.matrix().transform_point3(Vec3::ZERO),
        Vec3::new(1.0, 2.0, 3.0)
    );
    let s = Transform::from_scale(Vec3::new(2.0, 3.0, 4.0));
    assert_eq!(
        s.matrix().transform_point3(Vec3::ONE),
        Vec3::new(2.0, 3.0, 4.0)
    );
    let r = Transform::from_rotation(Quat::from_rotation_y(core::f32::consts::FRAC_PI_2));
    let p = r.matrix().transform_point3(Vec3::X);
    assert!(approx(p.x, 0.0) && approx(p.z, -1.0));
}

#[test]
fn mvp_composition() {
    let model = Transform::from_translation(Vec3::new(10.0, 0.0, 0.0)).matrix();
    let cam = Camera::new(
        Vec3::ZERO,
        Vec3::NEG_Z,
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 1.0,
            aspect: 1.0,
            near: 0.1,
            far: 100.0,
        },
    )
    .unwrap();
    let mvp = cam.projection_matrix() * cam.view_matrix() * model;
    // La cámara de este test está en el origen mirando hacia -Z, así que un
    // vértice en el volumen visible debe tener z negativa (por delante de la
    // cámara) y quedar dentro de los planos near..far (0.1..100).
    // Tras el desplazamiento del modelo (x += 10) el vértice queda en x = 5.
    let v = Vec4::new(-5.0, -5.0, -5.0, 1.0);
    let clip = mvp * v;
    assert!(
        clip.w > 0.0,
        "clip.w should be positive (vertex in front of camera)"
    );
    let ndc = Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);
    assert!(ndc.x.abs() < 2.0 && ndc.z >= -0.01 && ndc.z <= 1.01);
}

#[test]
fn perspective_near_far_zero_one() {
    let cam = Camera::new(
        Vec3::ZERO,
        Vec3::NEG_Z,
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 60.0_f32.to_radians(),
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 100.0,
        },
    )
    .unwrap();
    let p = cam.projection_matrix();
    let ndc = |z_view: f32| {
        let clip = p * Vec4::new(0.0, 0.0, z_view, 1.0);
        clip.z / clip.w
    };
    assert!(approx(ndc(-0.1), 0.0));
    assert!(approx(ndc(-100.0), 1.0));
    assert!(approx_mat(
        p,
        Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, 100.0)
    ));
}

#[test]
fn camera_rejects_invalid() {
    let mk = |fov: f32, aspect: f32, near: f32, far: f32| {
        Camera::new(
            Vec3::ZERO,
            Vec3::NEG_Z,
            Vec3::Y,
            Projection::Perspective {
                fov_y_rad: fov,
                aspect,
                near,
                far,
            },
        )
    };
    assert!(mk(0.0, 1.0, 0.1, 10.0).is_err());
    assert!(mk(1.0, 0.0, 0.1, 10.0).is_err());
    assert!(mk(1.0, 1.0, 10.0, 1.0).is_err());
    let mut cam = Camera::demo();
    assert!(cam.set_aspect(0.0).is_err());
    assert!(cam.set_aspect(4.0 / 3.0).is_ok());
}

#[test]
fn aabb_and_normals() {
    assert!(Aabb::new(Vec3::ZERO, Vec3::ONE).is_ok());
    assert!(Aabb::new(Vec3::ONE, Vec3::ZERO).is_err());
    assert!(Aabb::new(Vec3::new(f32::NAN, 0.0, 0.0), Vec3::ONE).is_err());
    let m = Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0));
    let n = transform_normal(m, Vec3::X);
    assert!(approx(n.x, 1.0) && approx(n.y, 0.0));
}

#[test]
fn frustum_culls_behind() {
    let cam = Camera::demo();
    let frustum = Frustum::from_view_projection(cam.view_projection_matrix());
    let visible = Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0)).unwrap();
    assert!(frustum.intersects_aabb(visible));
    let behind = Aabb::new(Vec3::new(-1.0, 100.0, 200.0), Vec3::new(1.0, 102.0, 202.0)).unwrap();
    assert!(!frustum.intersects_aabb(behind));
}

#[test]
fn engine_vec3_roundtrip() {
    let e = engine::Vec3 {
        x: 1.5,
        y: -2.0,
        z: 3.25,
    };
    let g = from_engine(e);
    assert_eq!((g.x, g.y, g.z), (1.5, -2.0, 3.25));
    assert_eq!(to_engine(g), e);
}
