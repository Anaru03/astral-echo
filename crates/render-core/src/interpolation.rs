use std::collections::VecDeque;

use crate::{RenderSnapshot, Transform};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InterpolationConfig {
    pub capacity: usize,
    pub delay: f32,
    pub teleport_distance: f32,
}

impl Default for InterpolationConfig {
    fn default() -> Self {
        Self {
            capacity: 8,
            delay: 1.0 / 60.0,
            teleport_distance: 25.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TimedSnapshot {
    time: f32,
    snapshot: RenderSnapshot,
}

#[derive(Clone, Debug)]
pub struct SnapshotBuffer {
    config: InterpolationConfig,
    snapshots: VecDeque<TimedSnapshot>,
}

impl SnapshotBuffer {
    pub fn new(config: InterpolationConfig) -> Self {
        Self {
            config: InterpolationConfig {
                capacity: config.capacity.max(2),
                ..config
            },
            snapshots: VecDeque::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn push(&mut self, snapshot: RenderSnapshot, presentation_time: f32) -> bool {
        if !presentation_time.is_finite() {
            return false;
        }
        if let Some(existing) = self
            .snapshots
            .iter_mut()
            .find(|item| item.time == presentation_time)
        {
            if snapshot.tick == existing.snapshot.tick {
                existing.snapshot = snapshot;
                return true;
            }
            return false;
        }
        if self
            .snapshots
            .back()
            .is_some_and(|item| presentation_time < item.time)
        {
            return false;
        }
        self.snapshots.push_back(TimedSnapshot {
            time: presentation_time,
            snapshot,
        });
        while self.snapshots.len() > self.config.capacity {
            self.snapshots.pop_front();
        }
        true
    }

    pub fn sample(&self, presentation_time: f32) -> Option<RenderSnapshot> {
        let target = presentation_time - self.config.delay;
        let first = self.snapshots.front()?;
        if target <= first.time {
            return Some(first.snapshot.clone());
        }
        let last = self.snapshots.back()?;
        if target >= last.time {
            return Some(last.snapshot.clone());
        }
        let (a, b) = self
            .snapshots
            .iter()
            .zip(self.snapshots.iter().skip(1))
            .find(|(a, b)| a.time <= target && target <= b.time)?;
        let alpha = ((target - a.time) / (b.time - a.time)).clamp(0.0, 1.0);
        Some(interpolate(
            &a.snapshot,
            &b.snapshot,
            alpha,
            self.config.teleport_distance,
        ))
    }
}

fn interpolate(
    a: &RenderSnapshot,
    b: &RenderSnapshot,
    alpha: f32,
    teleport_distance: f32,
) -> RenderSnapshot {
    let mut result = b.clone();
    for node in &mut result.nodes {
        let Some(previous) = a
            .nodes
            .iter()
            .find(|candidate| candidate.entity == node.entity)
        else {
            continue;
        };
        if previous.mesh != node.mesh || previous.material != node.material {
            continue;
        }
        node.transform =
            interpolate_transform(previous.transform, node.transform, alpha, teleport_distance);
    }
    result
}

fn interpolate_transform(
    a: Transform,
    b: Transform,
    alpha: f32,
    teleport_distance: f32,
) -> Transform {
    if (b.translation - a.translation).length() > teleport_distance {
        return b;
    }
    Transform::new(
        a.translation.lerp(b.translation, alpha),
        a.rotation.slerp(b.rotation, alpha).normalize(),
        a.scale.lerp(b.scale, alpha),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Camera, MaterialHandle, MeshHandle, SnapshotNode, Vec3};
    use engine::{EntityId, Tick};

    fn snapshot(tick: u64, x: f32) -> RenderSnapshot {
        let camera = Camera::demo();
        let mut snapshot = RenderSnapshot::new(
            Tick(tick),
            camera.position(),
            camera.view_matrix(),
            camera.projection_matrix(),
        );
        snapshot.push_node(SnapshotNode::new(
            EntityId::new(1),
            Transform::from_translation(Vec3::new(x, 0.0, 0.0)),
            MeshHandle::CUBE,
            MaterialHandle::new(0),
            true,
        ));
        snapshot
    }

    #[test]
    fn interpolates_position_and_is_deterministic() {
        let mut buffer = SnapshotBuffer::new(InterpolationConfig {
            delay: 0.0,
            ..Default::default()
        });
        buffer.push(snapshot(1, 0.0), 0.0);
        buffer.push(snapshot(2, 10.0), 1.0);
        let a = buffer.sample(0.5).unwrap();
        let b = buffer.sample(0.5).unwrap();
        assert_eq!(a, b);
        assert!((a.nodes[0].transform.translation.x - 5.0).abs() < 0.001);
    }

    #[test]
    fn rejects_out_of_order_and_bounds_memory() {
        let mut buffer = SnapshotBuffer::new(InterpolationConfig {
            capacity: 2,
            ..Default::default()
        });
        assert!(buffer.push(snapshot(1, 0.0), 1.0));
        assert!(!buffer.push(snapshot(0, 0.0), 0.0));
        buffer.push(snapshot(2, 1.0), 2.0);
        buffer.push(snapshot(3, 2.0), 3.0);
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn teleport_does_not_smear() {
        let mut buffer = SnapshotBuffer::new(InterpolationConfig {
            delay: 0.0,
            teleport_distance: 2.0,
            ..Default::default()
        });
        buffer.push(snapshot(1, 0.0), 0.0);
        buffer.push(snapshot(2, 10.0), 1.0);
        assert_eq!(
            buffer.sample(0.5).unwrap().nodes[0].transform.translation.x,
            10.0
        );
    }
}
