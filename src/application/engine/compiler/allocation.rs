use super::RenderStep;

/// Linear-scan lifetime allocation. Persistent external sources have dedicated slots.
pub(super) fn assign(
    steps: &mut [RenderStep],
    dependencies: &[[Option<usize>; 8]],
    retained: &[usize],
    final_step: Option<usize>,
) -> (usize, usize) {
    let mut last_use: Vec<_> = (0..steps.len()).collect();
    for (consumer, inputs) in dependencies.iter().enumerate() {
        for &producer in inputs.iter().flatten() {
            last_use[producer] = consumer;
        }
    }
    if let Some(final_step) = final_step {
        last_use[final_step] = steps.len();
    }
    for &i in retained {
        last_use[i] = steps.len();
    }
    let mut free = Vec::new();
    let mut count = 0;
    for index in 0..steps.len() {
        // External inputs own persistent storage; never alias a transient pool
        // target with storage the decoder may update before graph execution.
        let reusable = if steps[index].external_source() {
            None
        } else {
            free.pop()
        };
        let slot = reusable.unwrap_or_else(|| {
            let slot = count;
            count += 1;
            slot
        });
        steps[index].target = slot;
        steps[index].inputs = dependencies[index].map(|input| input.map(|i| steps[i].target));
        for (port, producer) in dependencies[index].iter().enumerate() {
            if let Some(producer) = producer
                && !steps[*producer].external_source()
                && last_use[*producer] == index
                && !dependencies[index][..port].contains(&Some(*producer))
            {
                free.push(steps[*producer].target);
            }
        }
        if last_use[index] == index && !steps[index].external_source() {
            free.push(slot);
        }
    }
    let output_slot = if let Some(final_step) = final_step {
        steps[final_step].target
    } else {
        count += 1;
        count - 1
    };
    (count, output_slot)
}
