//! The component crates linked into the engine, with their responsibilities (plan §3.1).

use serde::Serialize;

/// One linked component crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    /// Cargo package name.
    pub name: &'static str,
    /// Spec sections the crate is responsible for.
    pub spec_sections: &'static [&'static str],
    /// Milestone(s) that implement it.
    pub milestone: &'static str,
}

macro_rules! component {
    ($krate:ident) => {
        Component { name: $krate::NAME, spec_sections: $krate::SPEC_SECTIONS, milestone: $krate::MILESTONE }
    };
}

/// Every component crate linked into the engine, in dependency order, ending with the engine itself.
/// (`tt-mcp` depends on the engine, so the shells that link it add it themselves.)
#[must_use]
pub fn components() -> Vec<Component> {
    vec![
        component!(tt_model),
        component!(tt_registry),
        component!(tt_executor),
        component!(tt_vector),
        component!(tt_colour),
        component!(tt_anim),
        component!(tt_fx),
        component!(tt_render),
        component!(tt_audio),
        component!(tt_lipsync),
        component!(tt_persist),
        component!(tt_io),
        Component { name: crate::NAME, spec_sections: crate::SPEC_SECTIONS, milestone: crate::MILESTONE },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_every_plan_crate_once() {
        let names: Vec<_> = components().iter().map(|c| c.name).collect();
        let expected = [
            "tt-model",
            "tt-registry",
            "tt-executor",
            "tt-vector",
            "tt-colour",
            "tt-anim",
            "tt-fx",
            "tt-render",
            "tt-audio",
            "tt-lipsync",
            "tt-persist",
            "tt-io",
            "tt-engine",
        ];
        assert_eq!(names, expected);
    }

    #[test]
    fn every_component_cites_spec_sections() {
        for c in components() {
            assert!(!c.spec_sections.is_empty(), "{} has no spec sections", c.name);
            assert!(!c.milestone.is_empty(), "{} has no milestone", c.name);
        }
    }
}
