use std::any::TypeId;

#[test]
fn colored_mesh_types_are_shared_with_render_core() {
    assert_eq!(
        TypeId::of::<render_cpu::RasterMesh>(),
        TypeId::of::<render_core::RasterMesh>()
    );
    assert_eq!(
        TypeId::of::<render_cpu::RasterVertex>(),
        TypeId::of::<render_core::RasterVertex>()
    );
    let scene = render_core::demo_scene();
    assert_eq!(scene.len(), 19);
    assert_eq!(
        scene
            .iter()
            .map(|(mesh, _)| mesh.indices.len() / 3)
            .sum::<usize>(),
        278
    );
    assert_eq!(scene, render_cpu::demo_scene());
}

#[test]
fn cpu_camera_wrapper_preserves_shared_camera_and_error_contract() {
    for aspect in [1.0, 16.0 / 9.0, 2.0] {
        assert_eq!(
            render_cpu::demo_camera(aspect).unwrap(),
            render_core::demo_camera(aspect).unwrap()
        );
    }
    assert_eq!(
        render_cpu::demo_camera(0.0),
        Err(render_cpu::RasterError::InvalidViewport)
    );
    assert!(render_core::demo_camera(0.0).is_err());
}
