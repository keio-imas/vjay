use crate::registry::NodeRegistry;
mod arithmetic;
mod controls;
mod inputs;
mod timing;

pub(crate) fn register(registry: &mut NodeRegistry) {
    controls::register(registry);
    timing::register(registry);
    arithmetic::register(registry);
    inputs::register(registry);
}
