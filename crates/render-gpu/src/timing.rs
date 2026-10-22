use crate::{GpuContext, GpuError};

#[derive(Clone, Copy, Debug)]
pub struct FrameGpuTimings {
    pub shadow_ms: f64,
    pub color_ms: f64,
}

pub(crate) struct GpuTimer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    staging: wgpu::Buffer,
    shadow_active: bool,
    submitted: bool,
}

impl GpuTimer {
    pub fn new(gpu: &GpuContext) -> Option<Self> {
        if !gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            return None;
        }
        let queries = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("render pass timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: 4,
        });
        let resolve = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamp resolve"),
            size: 32,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamp readback"),
            size: 32,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Some(Self {
            queries,
            resolve,
            staging,
            shadow_active: false,
            submitted: false,
        })
    }

    pub fn writes(&self, shadow: bool) -> wgpu::RenderPassTimestampWrites<'_> {
        let first = if shadow { 0 } else { 2 };
        wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(first),
            end_of_pass_write_index: Some(first + 1),
        }
    }

    pub fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder, shadow_active: bool) {
        self.shadow_active = shadow_active;
        self.submitted = true;
        let first = if shadow_active { 0 } else { 2 };
        encoder.resolve_query_set(&self.queries, first..4, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.staging, 0, u64::from(4 - first) * 8);
    }

    pub fn read(&self, gpu: &GpuContext) -> Result<Option<FrameGpuTimings>, GpuError> {
        if !self.submitted {
            return Ok(None);
        }
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        gpu.wait()?;
        rx.recv_timeout(std::time::Duration::from_secs(20))
            .map_err(|e| GpuError::Readback(e.to_string()))?
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        let mapped = self.staging.slice(..).get_mapped_range();
        let values: Vec<_> = mapped
            .chunks_exact(8)
            .map(|bytes| u64::from_le_bytes(bytes.try_into().expect("timestamp bytes")))
            .collect();
        let period = f64::from(gpu.queue.get_timestamp_period()) / 1_000_000.0;
        let (shadow_ms, color_ms) = if self.shadow_active {
            (
                (values[1].saturating_sub(values[0])) as f64 * period,
                (values[3].saturating_sub(values[2])) as f64 * period,
            )
        } else {
            (0.0, (values[1].saturating_sub(values[0])) as f64 * period)
        };
        drop(mapped);
        self.staging.unmap();
        Ok(Some(FrameGpuTimings {
            shadow_ms,
            color_ms,
        }))
    }
}
