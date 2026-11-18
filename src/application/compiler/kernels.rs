use super::*;
pub(super) fn validate(operation: &ImageKernel, parameters: usize) -> PortResult<()> {
    if !operation.selector.is_finite()
        || operation.parameters.len() > 64
        || operation.parameters.len() != operation.parameter_slots.len()
        || operation.parameters.iter().any(|v| !v.is_finite())
        || operation
            .parameter_slots
            .iter()
            .flatten()
            .any(|&i| i >= parameters)
    {
        return Err("Invalid GPU uniform mapping".into());
    }
    Ok(())
}
