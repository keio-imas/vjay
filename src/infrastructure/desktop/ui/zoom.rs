use super::*;
impl Editor {
    pub(super) fn zoom_at(&mut self, pointer: Position, factor: f32) {
        let Some(world) = self.world_pointer(pointer) else {
            return;
        };
        let next = (self.zoom * factor).clamp(0.25, 4.0);
        self.pan.x += world.x * (self.zoom - next);
        self.pan.y += world.y * (self.zoom - next);
        self.zoom = next;
        let mut layout = self.session().project().workspace;
        layout.patch_zoom = next;
        let result = self.session_mut().set_workspace(layout);
        self.apply(result);
    }
}
