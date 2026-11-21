use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShaderUniform {
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}
impl ShaderUniform {
    pub fn is_valid(&self) -> bool {
        let mut bytes = self.name.bytes();
        let first = bytes.next();
        first.is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
            && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && self.name.len() <= 64
            && !self.name.starts_with("gl_")
            && !self.name.starts_with("iChannel")
            && ![
                "iTime",
                "iResolution",
                "iParameters",
                "frame",
                "inputA",
                "inputB",
                "inputSampler",
                "main",
                "mainImage",
                "float",
                "int",
                "bool",
                "vec2",
                "vec3",
                "vec4",
                "sin",
                "cos",
                "texture",
            ]
            .contains(&self.name.as_str())
            && self.min.is_finite()
            && self.max.is_finite()
            && self.min < self.max
            && self.default.is_finite()
            && (self.min..=self.max).contains(&self.default)
    }
}

/// Rename whole GLSL tokens in one pass, preserving comments, quoted paths and
/// longer identifiers. This also handles simultaneous name swaps.
pub fn rewrite_identifiers(source: &str, names: &[(&str, &str)]) -> String {
    let bytes = source.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        if bytes[i..].starts_with(b"//") {
            i = bytes[i..]
                .iter()
                .position(|b| *b == b'\n')
                .map_or(bytes.len(), |n| i + n);
        } else if bytes[i..].starts_with(b"/*") {
            i = bytes[i + 2..]
                .windows(2)
                .position(|p| p == b"*/")
                .map_or(bytes.len(), |n| i + 2 + n + 2);
        } else if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(bytes.len());
                } else if bytes[i] == quote {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
        } else if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let token = &source[start..i];
            if let Some((_, name)) = names.iter().find(|(name, _)| *name == token) {
                result.extend_from_slice(name.as_bytes());
                continue;
            }
        } else {
            i += 1;
        }
        result.extend_from_slice(&bytes[start..i]);
    }
    String::from_utf8(result).expect("source and replacement identifiers are UTF-8")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renaming_preserves_comments_strings_and_identifier_boundaries() {
        let source = "// gain は説明\n/* gain */\n#define path \"gain.glsl\"\nfloat x=gain+gain_other+level;";
        let result = rewrite_identifiers(source, &[("gain", "level"), ("level", "gain")]);
        assert!(result.contains("// gain は説明"));
        assert!(result.contains("\"gain.glsl\""));
        assert!(result.contains("x=level+gain_other+gain;"));
    }
}
