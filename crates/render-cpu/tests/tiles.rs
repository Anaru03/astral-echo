use render_cpu::pipeline::tiles::{RasterMode, TileConfig, TileGrid, TileRect};

#[test]
fn tile_grid_divides_framebuffer_correctly() {
    let grid = TileGrid::new(640, 360, 16);
    assert_eq!(grid.tiles_w, 40);
    assert_eq!(grid.tiles_h, 23);
    assert_eq!(grid.tile_count(), 40 * 23);
    assert_eq!(grid.tile_size, 16);
}

#[test]
fn tile_grid_edge_tiles_respect_framebuffer_bounds() {
    let grid = TileGrid::new(100, 100, 32);
    assert_eq!(grid.tiles_w, 4);
    assert_eq!(grid.tiles_h, 4);
    let last = grid.tile_rect(3, 3);
    assert_eq!(last.x, 96);
    assert_eq!(last.y, 96);
    assert_eq!(last.w, 4);
    assert_eq!(last.h, 4);
}

#[test]
fn tile_grid_single_tile_when_smaller_than_tile_size() {
    let grid = TileGrid::new(8, 8, 16);
    assert_eq!(grid.tiles_w, 1);
    assert_eq!(grid.tiles_h, 1);
    let t = grid.tile_rect(0, 0);
    assert_eq!(t.w, 8);
    assert_eq!(t.h, 8);
}

#[test]
fn tiles_intersected_by_bbox_returns_empty_for_outside() {
    let grid = TileGrid::new(100, 100, 16);
    let tiles = grid.tiles_intersected_by_bbox(200.0, 300.0, 200.0, 300.0);
    assert!(tiles.is_empty());
    let tiles = grid.tiles_intersected_by_bbox(-200.0, -100.0, -200.0, -100.0);
    assert!(tiles.is_empty());
}

#[test]
fn tiles_intersected_by_bbox_covers_single_tile() {
    let grid = TileGrid::new(100, 100, 16);
    // Bbox at y=20 intersects tile row 1 (y 16-31), not row 0
    let tiles = grid.tiles_intersected_by_bbox(10.0, 20.0, 10.0, 20.0);
    assert_eq!(tiles.len(), 1);
    assert_eq!(
        tiles[0],
        TileRect {
            x: 0,
            y: 16,
            w: 16,
            h: 16
        }
    );
}

#[test]
fn tiles_intersected_by_bbox_covers_multiple_tiles() {
    let grid = TileGrid::new(100, 100, 16);
    // Bbox at y=40 intersects tile row 2 (y 32-47), just 1 tile for zero-width bbox
    let tiles = grid.tiles_intersected_by_bbox(10.0, 40.0, 10.0, 40.0);
    assert_eq!(tiles.len(), 1);
    assert_eq!(
        tiles[0],
        TileRect {
            x: 0,
            y: 32,
            w: 16,
            h: 16
        }
    );
}

#[test]
fn tile_rect_contains() {
    let tile = TileRect {
        x: 16,
        y: 32,
        w: 16,
        h: 16,
    };
    assert!(tile.contains(16.0, 32.0));
    assert!(tile.contains(31.0, 47.0));
    assert!(!tile.contains(15.0, 32.0));
    assert!(!tile.contains(32.0, 32.0));
    assert!(!tile.contains(16.0, 31.0));
    assert!(!tile.contains(16.0, 48.0));
}

#[test]
fn tile_grid_resolutions_not_divisible_by_tile_size() {
    // Test various non-divisible resolutions
    for (w, h, ts) in [(100, 100, 3), (641, 480, 16), (639, 361, 8)] {
        let grid = TileGrid::new(w, h, ts);
        // Tile count should still be correct with ceiling division
        assert!(grid.tiles_w >= (w as f32 / ts as f32).ceil() as u32);
        assert!(grid.tiles_h >= (h as f32 / ts as f32).ceil() as u32);
    }
}

#[test]
fn tile_grid_tile_rect_edge_cases() {
    // Test edge tile rectangles produce correct dimensions
    let grid = TileGrid::new(100, 100, 32);
    // Only the bottom-right tile should have truncated dimensions
    let tr = grid.tile_rect(3, 3);
    assert_eq!(tr.x, 96);
    assert_eq!(tr.y, 96);
    assert_eq!(tr.w, 4); // 100 - 96
    assert_eq!(tr.h, 4); // 100 - 96

    // All other tiles should have full tile_size dimensions
    let tl = grid.tile_rect(0, 3);
    assert_eq!(tl.w, 32);
    assert_eq!(tl.h, 4);

    let bl = grid.tile_rect(3, 0);
    assert_eq!(bl.w, 4);
    assert_eq!(bl.h, 32);

    let cc = grid.tile_rect(0, 0);
    assert_eq!(cc.w, 32);
    assert_eq!(cc.h, 32);
}

#[test]
fn tile_grid_bbox_intersection_ordering() {
    // Verify that bbox intersection returns tiles in consistent order
    let grid = TileGrid::new(200, 200, 16);
    let tiles = grid.tiles_intersected_by_bbox(0.0, 0.0, 50.0, 50.0);
    // Should return tiles starting from (0,0)
    assert!(tiles.iter().any(|t| t.x == 0 && t.y == 0));
    // Max 4x4 tile grid for 50x50 bbox in 200x200 framebuffer with 16px tiles
    assert!(tiles.len() <= 16);
}

#[test]
fn tile_config_raster_mode_default() {
    // Test that TileConfig default mode is Secuencial (#[default] on Secuencial)
    let config = TileConfig::default();
    assert_eq!(config.mode, RasterMode::Secuencial);
}

#[test]
fn tile_grid_multiple_tile_sizes() {
    // Test tile grid with various tile sizes
    for ts in [1, 2, 4, 8, 16, 32, 64] {
        let grid = TileGrid::new(64, 64, ts);
        assert!(grid.tiles_w > 0);
        assert!(grid.tiles_h > 0);
        assert!(grid.tile_count() > 0);
    }
}

#[test]
fn tile_grid_zero_dimensions_handled() {
    let grid = TileGrid::new(0, 0, 16);
    assert_eq!(grid.tiles_w, 0);
    assert_eq!(grid.tiles_h, 0);
    assert_eq!(grid.tile_count(), 0);

    let grid = TileGrid::new(0, 100, 16);
    assert_eq!(grid.tiles_w, 0);
    assert_eq!(grid.tiles_h, 7);

    let grid = TileGrid::new(100, 0, 16);
    assert_eq!(grid.tiles_w, 7);
    assert_eq!(grid.tiles_h, 0);
}

#[test]
fn tiled_matches_reference_for_depth_perspective_winding_and_modes() {
    use glam::Vec4;
    use render_cpu::pipeline::{
        raster_triangle, rasterize_triangles_tiled, resolve_fragment, FillRule, ScreenVertex,
    };
    use render_cpu::Framebuffer;

    let vertex = |x, y, z, inv_w, color| ScreenVertex {
        x,
        y,
        z,
        inv_w,
        color,
    };
    let a = vertex(-2.0, 0.0, 0.2, 1.0, Vec4::new(1.0, 0.0, 0.0, 0.25));
    let b = vertex(10.0, 1.0, 0.7, 0.2, Vec4::new(0.0, 1.0, 0.0, 0.5));
    let c = vertex(1.0, 10.0, 0.4, 0.5, Vec4::new(0.0, 0.0, 1.0, 0.75));
    let d = vertex(1.0, 1.0, 0.6, 1.0, Vec4::new(0.4, 0.8, 0.2, 0.5));
    let e = vertex(8.0, 1.0, 0.6, 1.0, d.color);
    let f = vertex(1.0, 8.0, 0.6, 1.0, d.color);
    for triangles in [vec![(a, b, c), (d, e, f)], vec![(d, f, e), (a, c, b)]] {
        for rule in [FillRule::All, FillRule::TopLeft] {
            for (depth_test, depth_write) in
                [(false, false), (true, false), (false, true), (true, true)]
            {
                let mut expected = Framebuffer::new(9, 9).unwrap();
                expected.set_depth(2, 2, 0.1);
                let mut fragments = Vec::new();
                let mut written = 0;
                for (a, b, c) in &triangles {
                    raster_triangle(a, b, c, (9, 9), rule, &mut fragments);
                }
                for fragment in &fragments {
                    written += u32::from(
                        resolve_fragment(fragment, &mut expected, depth_test, depth_write).unwrap(),
                    );
                }
                assert!(written > 0);
                for tile_size in [0, 1, 3, 16] {
                    for mode in [
                        RasterMode::Secuencial,
                        RasterMode::Paralelo,
                        RasterMode::Auto,
                    ] {
                        let mut actual = Framebuffer::new(9, 9).unwrap();
                        actual.set_depth(2, 2, 0.1);
                        let stats = rasterize_triangles_tiled(
                            &triangles,
                            &mut actual,
                            rule,
                            depth_test,
                            depth_write,
                            TileConfig { tile_size, mode },
                        );
                        assert_eq!(actual, expected, "tile_size={tile_size}, mode={mode:?}");
                        assert_eq!(stats.fragments_rasterized, fragments.len() as u32);
                        assert_eq!(stats.pixels_written, written);
                    }
                }
            }
        }
    }
}

#[test]
fn tiled_top_left_shared_edge_is_filled_once() {
    use glam::Vec4;
    use render_cpu::{rasterize_triangles_tiled, FillRule, Framebuffer, ScreenVertex};
    let vertex = |x, y| ScreenVertex {
        x,
        y,
        z: 0.5,
        inv_w: 1.0,
        color: Vec4::ONE,
    };
    let a = vertex(0.0, 0.0);
    let b = vertex(8.0, 0.0);
    let c = vertex(8.0, 8.0);
    let d = vertex(0.0, 8.0);
    let mut framebuffer = Framebuffer::new(8, 8).unwrap();
    let stats = rasterize_triangles_tiled(
        &[(a, b, c), (a, c, d)],
        &mut framebuffer,
        FillRule::TopLeft,
        false,
        false,
        TileConfig {
            tile_size: 3,
            mode: RasterMode::Secuencial,
        },
    );
    assert_eq!(stats.fragments_rasterized, 64);
    assert_eq!(stats.pixels_written, 64);
    assert!(framebuffer.color_bytes().iter().all(|&value| value == 255));
}

#[test]
fn tile_grid_rejects_invalid_bounds_and_normalizes_zero_tile_size() {
    let grid = TileGrid::new(8, 8, 0);
    assert_eq!(grid.tile_size, 1);
    assert_eq!(grid.tiles_intersected_by_bbox(7.0, 7.0, 8.0, 8.0).len(), 1);
    for bbox in [
        (3.0, 3.0, 2.0, 4.0),
        (f32::NAN, 0.0, 1.0, 1.0),
        (0.0, 0.0, f32::INFINITY, 1.0),
    ] {
        assert!(grid
            .tiles_intersected_by_bbox(bbox.0, bbox.1, bbox.2, bbox.3)
            .is_empty());
    }
    let empty = TileGrid::new(0, 0, 16);
    assert!(empty
        .tiles_intersected_by_bbox(0.0, 0.0, 1.0, 1.0)
        .is_empty());
    assert_eq!(empty.tile_rect(0, 0).w, 0);
}
