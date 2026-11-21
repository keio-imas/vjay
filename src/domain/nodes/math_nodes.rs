use crate::{
    builtins::SignalNode,
    capabilities::Capabilities as C,
    execution::ScalarContext,
    node::{NodeDescriptor as D, Parameter as P},
    port::Port,
    registry::NodeRegistry,
};
mod easing;
const OUT: &[Port] = &[Port::scalar("Value")];
const BINARY: &[P] = &[P::new("A", -1e6, 1e6, 0.0), P::new("B", -1e6, 1e6, 1.0)];
const UNARY: &[P] = &[P::new("Value", -1e6, 1e6, 0.0)];
macro_rules! math {
    ($r:ident,$id:literal,$name:literal,$params:expr,$evaluate:expr) => {{
        static D: D = D {
            type_id: $id,
            name: $name,
            category: "Math",
            inputs: &[],
            outputs: OUT,
            parameters: $params,
            capabilities: C::NONE,
        };
        $r.register(SignalNode {
            descriptor: &D,
            evaluate: $evaluate,
            cells: 0,
            tap: None,
        })
        .expect("valid math node");
    }};
}
pub(crate) fn register(r: &mut NodeRegistry) {
    math!(r, "Subtract", "Subtract", BINARY, |c, _, o| o[0] =
        c.parameters[0] - c.parameters[1]);
    math!(
        r,
        "Divide",
        "Divide",
        BINARY,
        |c, _, o| o[0] = if c.parameters[1].abs() > 1e-12 {
            c.parameters[0] / c.parameters[1]
        } else {
            0.0
        }
    );
    math!(
        r,
        "Modulo",
        "Modulo",
        BINARY,
        |c, _, o| o[0] = if c.parameters[1].abs() > 1e-12 {
            c.parameters[0].rem_euclid(c.parameters[1].abs())
        } else {
            0.0
        }
    );
    math!(r, "Power", "Power", BINARY, |c, _, o| o[0] =
        c.parameters[0].powf(c.parameters[1]));
    math!(r, "Minimum", "Minimum", BINARY, |c, _, o| o[0] =
        c.parameters[0].min(c.parameters[1]));
    math!(r, "Maximum", "Maximum", BINARY, |c, _, o| o[0] =
        c.parameters[0].max(c.parameters[1]));
    math!(r, "Absolute", "Absolute", UNARY, |c, _, o| o[0] =
        c.parameters[0].abs());
    math!(r, "Sine", "Sine (radians)", UNARY, |c, _, o| o[0] =
        c.parameters[0].sin());
    math!(r, "Cosine", "Cosine (radians)", UNARY, |c, _, o| o[0] =
        c.parameters[0].cos());
    math!(r, "Floor", "Floor", UNARY, |c, _, o| o[0] =
        c.parameters[0].floor());
    math!(r, "Round", "Round", UNARY, |c, _, o| o[0] =
        c.parameters[0].round());
    math!(r, "Fraction", "Fraction", UNARY, |c, _, o| o[0] =
        c.parameters[0].rem_euclid(1.0));
    math!(
        r,
        "Compare",
        "Compare",
        &[
            P::new("A", -1e6, 1e6, 0.0),
            P::new("B", -1e6, 1e6, 0.0),
            P::new("Mode", 0.0, 5.0, 0.0).choices(crate::choices::COMPARE)
        ],
        |c, _, o| {
            let [a, b, m] = [c.parameters[0], c.parameters[1], c.parameters[2]];
            o[0] = f32::from(match m.round() as u32 {
                0 => a < b,
                1 => a <= b,
                2 => (a - b).abs() < 1e-6,
                3 => a >= b,
                4 => a > b,
                _ => (a - b).abs() >= 1e-6,
            });
        }
    );
    math!(r, "LogicAnd", "AND", BINARY, |c, _, o| o[0] =
        f32::from(c.parameters[0] > 0.5 && c.parameters[1] > 0.5));
    math!(r, "LogicOr", "OR", BINARY, |c, _, o| o[0] =
        f32::from(c.parameters[0] > 0.5 || c.parameters[1] > 0.5));
    math!(r, "LogicNot", "NOT", UNARY, |c, _, o| o[0] =
        f32::from(c.parameters[0] <= 0.5));
    math!(
        r,
        "Quantize",
        "Quantize",
        &[
            P::new("Value", -1e6, 1e6, 0.0),
            P::new("Step", 0.00001, 1e6, 1.0)
        ],
        |c, _, o| o[0] = (c.parameters[0] / c.parameters[1]).round() * c.parameters[1]
    );
    math!(
        r,
        "Easing",
        "Easing",
        &[
            P::new("Value", 0.0, 1.0, 0.0),
            P::new("Curve", 0.0, 20.0, 3.0).choices(crate::choices::EASING)
        ],
        easing::evaluate
    );
    static SELECT: D = D {
        type_id: "ValueSelector",
        name: "Value selector",
        category: "Signals",
        inputs: &[
            Port::scalar("1").optional(),
            Port::scalar("2").optional(),
            Port::scalar("3").optional(),
            Port::scalar("4").optional(),
            Port::scalar("5").optional(),
            Port::scalar("6").optional(),
            Port::scalar("7").optional(),
            Port::scalar("8").optional(),
        ],
        outputs: OUT,
        parameters: &[P::new("Index", 0.0, 7.0, 0.0).choices(crate::choices::INPUTS)],
        capabilities: C::NONE,
    };
    r.register(SignalNode {
        descriptor: &SELECT,
        evaluate: |c, _, o| o[0] = c.inputs[c.parameters[0].round() as usize].unwrap_or(0.0),
        cells: 0,
        tap: None,
    })
    .expect("valid selector");
    static HOLD: D = D {
        type_id: "SampleHold",
        name: "Sample and hold",
        category: "Signals",
        inputs: &[
            Port::scalar("Value").optional(),
            Port::scalar("Trigger").optional(),
        ],
        outputs: OUT,
        parameters: &[],
        capabilities: C::NONE,
    };
    r.register(SignalNode {
        descriptor: &HOLD,
        evaluate: |c, s, o| {
            let gate = c.inputs[1].unwrap_or(0.0) > 0.5;
            if gate && s[0] <= 0.5 {
                s[1] = c.inputs[0].unwrap_or(0.0) as f64;
            }
            s[0] = f64::from(gate);
            o[0] = s[1] as f32;
        },
        cells: 2,
        tap: None,
    })
    .expect("valid hold");
    static ENVELOPE: D = D {
        type_id: "Envelope",
        name: "Attack / Release",
        category: "Signals",
        inputs: &[Port::scalar("Gate").optional()],
        outputs: OUT,
        parameters: &[
            P::new("Attack seconds", 0.0, 30.0, 0.1),
            P::new("Release seconds", 0.0, 30.0, 0.3),
        ],
        capabilities: C::NONE,
    };
    r.register(SignalNode {
        descriptor: &ENVELOPE,
        evaluate: envelope,
        cells: 2,
        tap: None,
    })
    .expect("valid envelope");
}
fn envelope(c: &ScalarContext<'_>, s: &mut [f64], o: &mut [f32]) {
    let gate = c.inputs[0].unwrap_or(0.0) > 0.5 || s[1] > 0.5;
    let duration = c.parameters[usize::from(!gate)] as f64;
    if duration <= 0.0 {
        s[0] = f64::from(gate);
        s[1] = 0.0;
        o[0] = s[0] as f32;
        return;
    }
    // Preserve a pulse sampled at initialization or a repeated timestamp until
    // elapsed time allows its attack to be evaluated.
    if c.delta <= 0.0 {
        s[1] = f64::from(gate);
        o[0] = s[0] as f32;
        return;
    }
    s[1] = 0.0;
    let delta = c.delta / duration;
    s[0] = if gate {
        (s[0] + delta).min(1.0)
    } else {
        (s[0] - delta).max(0.0)
    };
    o[0] = s[0] as f32;
}

#[cfg(test)]
mod tests;
