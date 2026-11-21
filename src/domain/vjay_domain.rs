//! Backend-independent contracts and open node registrations.
pub mod graph;
pub mod node;
pub mod nodes;
pub mod transport;
// Stable public facade for existing node providers and project clients.
pub use graph::composition;
pub use node::{capabilities, choices, execution, port, registry, schema, settings, uniform};
pub use nodes::builtins;
pub use transport::tempo;
