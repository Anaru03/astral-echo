use crate::error::RasterError;
use crate::geometry::RasterVertex;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Topology {
    TriangleList,
    TriangleStrip,
    TriangleFan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Triangle {
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

impl Triangle {
    pub const fn new(a: u32, b: u32, c: u32) -> Self {
        Self { a, b, c }
    }

    pub fn indices(&self) -> [u32; 3] {
        [self.a, self.b, self.c]
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AssemblyStats {
    pub triangles: u32,
    pub degenerate: u32,
    pub invalid_indices: u32,
    pub out_of_range: u32,
}

pub fn assemble(
    topology: Topology,
    indices: &[u32],
    vertex_count: u32,
    out: &mut Vec<Triangle>,
) -> Result<AssemblyStats, RasterError> {
    out.clear();
    let mut stats = AssemblyStats::default();
    match topology {
        Topology::TriangleList => {
            for chunk in indices.chunks_exact(3) {
                push_triangle(chunk[0], chunk[1], chunk[2], vertex_count, &mut stats, out);
            }
            if !indices.len().is_multiple_of(3) {
                stats.invalid_indices += 1;
            }
        }
        Topology::TriangleStrip => {
            if indices.len() < 3 {
                return Ok(stats);
            }
            for i in 0..indices.len() - 2 {
                let (a, b, c) = if i % 2 == 0 {
                    (indices[i], indices[i + 1], indices[i + 2])
                } else {
                    (indices[i + 1], indices[i], indices[i + 2])
                };
                push_triangle(a, b, c, vertex_count, &mut stats, out);
            }
        }
        Topology::TriangleFan => {
            if indices.len() < 3 {
                return Ok(stats);
            }
            for i in 1..indices.len() - 1 {
                push_triangle(
                    indices[0],
                    indices[i],
                    indices[i + 1],
                    vertex_count,
                    &mut stats,
                    out,
                );
            }
        }
    }
    Ok(stats)
}

fn push_triangle(
    a: u32,
    b: u32,
    c: u32,
    vertex_count: u32,
    stats: &mut AssemblyStats,
    out: &mut Vec<Triangle>,
) {
    if a >= vertex_count || b >= vertex_count || c >= vertex_count {
        stats.out_of_range += 1;
        return;
    }
    if a == b || b == c || a == c {
        stats.degenerate += 1;
        return;
    }
    stats.triangles += 1;
    out.push(Triangle::new(a, b, c));
}

pub fn vertex_at<'a>(vertices: &'a [RasterVertex], index: u32) -> Option<&'a RasterVertex> {
    vertices.get(index as usize)
}
