use glam::{Mat4, Vec3, Vec4};
use render_cpu::pipeline::assembly::assemble;
use render_cpu::pipeline::vertex::{model_to_clip, model_to_world, world_to_view, VertexStage};
use render_cpu::{RasterError, RasterVertex, Topology};

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn vertex(x: f32, y: f32, z: f32) -> RasterVertex {
    RasterVertex::new(Vec3::new(x, y, z), Vec4::ONE)
}

#[test]
fn identity_mvp_keeps_position_in_clip_space() {
    let stage = VertexStage::new(Mat4::IDENTITY);
    let out = stage.transform(&vertex(1.0, 2.0, 3.0)).expect("transforms");
    assert_eq!(out.clip, Vec4::new(1.0, 2.0, 3.0, 1.0));
}

#[test]
fn model_matrix_is_applied_to_position() {
    let model = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
    let stage = VertexStage::new(model);
    let out = stage.transform(&vertex(1.0, 0.0, 0.0)).expect("transforms");
    assert_eq!(out.clip.x, 6.0);
}

#[test]
fn view_space_uses_negative_z_forward() {
    let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
    let p = world_to_view(Vec3::new(0.0, 0.0, -5.0), &view);
    assert!(approx(p.z, -5.0));
    assert!(approx(p.x, 0.0) && approx(p.y, 0.0));
}

#[test]
fn model_to_world_matches_transform_point() {
    let model = Mat4::from_scale_rotation_translation(
        Vec3::new(2.0, 2.0, 2.0),
        glam::Quat::from_rotation_y(0.5),
        Vec3::new(1.0, 0.0, 0.0),
    );
    let p = model_to_world(Vec3::new(1.0, 0.0, 0.0), &model);
    let expected = model.transform_point3(Vec3::new(1.0, 0.0, 0.0));
    assert!(approx(p.x, expected.x) && approx(p.y, expected.y) && approx(p.z, expected.z));
}

#[test]
fn clip_position_uses_full_mvp_chain() {
    let mvp = Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0, //
    ]);
    let out = model_to_clip(Vec3::new(1.0, 2.0, 3.0), &mvp);
    assert_eq!(out, Vec4::new(1.0, 2.0, 3.0, 1.0));
}

#[test]
fn vertex_attributes_are_preserved() {
    let stage = VertexStage::new(Mat4::IDENTITY);
    let v = RasterVertex::new(Vec3::ZERO, Vec4::new(0.1, 0.2, 0.3, 0.4));
    let out = stage.transform(&v).expect("transforms");
    assert_eq!(out.color, Vec4::new(0.1, 0.2, 0.3, 0.4));
}

#[test]
fn non_finite_position_is_rejected() {
    let stage = VertexStage::new(Mat4::IDENTITY);
    let v = RasterVertex::new(Vec3::new(f32::NAN, 0.0, 0.0), Vec4::ONE);
    assert_eq!(stage.transform(&v).unwrap_err(), RasterError::NonFiniteVertex);
}

#[test]
fn non_finite_color_is_rejected() {
    let stage = VertexStage::new(Mat4::IDENTITY);
    let v = RasterVertex::new(Vec3::ZERO, Vec4::new(f32::INFINITY, 0.0, 0.0, 1.0));
    assert_eq!(stage.transform(&v).unwrap_err(), RasterError::NonFiniteVertex);
}

#[test]
fn non_finite_result_is_rejected() {
    let mvp = Mat4::from_cols_array(&[
        f32::INFINITY, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0, //
    ]);
    let stage = VertexStage::new(mvp);
    assert_eq!(
        stage.transform(&vertex(1.0, 0.0, 0.0)).unwrap_err(),
        RasterError::NonFiniteVertex
    );
}

#[test]
fn transform_into_reuses_buffer() {
    let stage = VertexStage::new(Mat4::IDENTITY);
    let vertices = vec![vertex(0.0, 0.0, 0.0), vertex(1.0, 0.0, 0.0)];
    let mut out = Vec::new();
    stage.transform_into(&vertices, &mut out).expect("transforms");
    assert_eq!(out.len(), 2);
    stage.transform_into(&vertices, &mut out).expect("transforms");
    assert_eq!(out.len(), 2);
}

#[test]
fn triangle_list_assembles_in_order() {
    let mut out = Vec::new();
    let stats = assemble(
        Topology::TriangleList,
        &[0, 1, 2, 3, 4, 5],
        6,
        &mut out,
    )
    .expect("assembles");
    assert_eq!(stats.triangles, 2);
    assert_eq!(out[0].indices(), [0, 1, 2]);
    assert_eq!(out[1].indices(), [3, 4, 5]);
}

#[test]
fn triangle_list_rejects_incomplete_group() {
    let mut out = Vec::new();
    let stats = assemble(Topology::TriangleList, &[0, 1, 2, 3], 4, &mut out).expect("assembles");
    assert_eq!(stats.triangles, 1);
    assert_eq!(stats.invalid_indices, 1);
}

#[test]
fn triangle_strip_alternates_winding() {
    let mut out = Vec::new();
    let stats = assemble(Topology::TriangleStrip, &[0, 1, 2, 3, 4], 5, &mut out).expect("assembles");
    assert_eq!(stats.triangles, 3);
    assert_eq!(out[0].indices(), [0, 1, 2]);
    assert_eq!(out[1].indices(), [2, 1, 3]);
    assert_eq!(out[2].indices(), [2, 3, 4]);
}

#[test]
fn triangle_fan_shares_first_vertex() {
    let mut out = Vec::new();
    let stats = assemble(Topology::TriangleFan, &[0, 1, 2, 3], 4, &mut out).expect("assembles");
    assert_eq!(stats.triangles, 2);
    assert_eq!(out[0].indices(), [0, 1, 2]);
    assert_eq!(out[1].indices(), [0, 2, 3]);
}

#[test]
fn out_of_range_indices_are_rejected() {
    let mut out = Vec::new();
    let stats = assemble(Topology::TriangleList, &[0, 1, 9], 3, &mut out).expect("assembles");
    assert_eq!(stats.triangles, 0);
    assert_eq!(stats.out_of_range, 1);
    assert!(out.is_empty());
}

#[test]
fn repeated_indices_are_degenerate() {
    let mut out = Vec::new();
    let stats = assemble(Topology::TriangleList, &[0, 1, 1], 3, &mut out).expect("assembles");
    assert_eq!(stats.triangles, 0);
    assert_eq!(stats.degenerate, 1);
}

#[test]
fn topology_with_too_few_indices_yields_nothing() {
    let mut out = Vec::new();
    for topology in [
        Topology::TriangleList,
        Topology::TriangleStrip,
        Topology::TriangleFan,
    ] {
        let stats = assemble(topology, &[0, 1], 4, &mut out).expect("assembles");
        assert_eq!(stats.triangles, 0);
        assert!(out.is_empty());
    }
}

#[test]
fn assembly_clears_previous_output() {
    let mut out = Vec::new();
    assemble(Topology::TriangleList, &[0, 1, 2], 3, &mut out).expect("assembles");
    assert_eq!(out.len(), 1);
    assemble(Topology::TriangleList, &[], 3, &mut out).expect("assembles");
    assert!(out.is_empty());
}
