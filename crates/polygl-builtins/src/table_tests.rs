use crate::{BuiltinTable, BuiltinTier, BuiltinType, Domain};

#[test]
fn registry_is_valid_and_contains_every_tier_one_function() {
    BuiltinTable::validate().unwrap();
    assert_eq!(
        BuiltinTable::all()
            .iter()
            .map(|builtin| builtin.id)
            .collect::<Vec<_>>(),
        crate::BuiltinId::ALL
    );
    let tier_one = BuiltinTable::all()
        .iter()
        .filter(|builtin| builtin.tier == BuiltinTier::Tier1)
        .map(|builtin| builtin.name)
        .collect::<Vec<_>>();
    assert_eq!(
        tier_one,
        [
            "size",
            "background",
            "fill",
            "stroke",
            "no_stroke",
            "rect",
            "circle",
            "line",
            "triangle",
            "text",
            "push_matrix",
            "pop_matrix",
            "translate",
            "rotate",
            "scale",
            "width",
            "height",
            "time",
            "mouse_x",
            "mouse_y",
            "key_down",
            "random",
        ]
    );
    assert_eq!(
        BuiltinTable::find("material_shader").unwrap().tier,
        BuiltinTier::Tier2
    );
}

#[test]
fn domains_and_defaults_match_the_public_contract() {
    assert_eq!(BuiltinTable::find("time").unwrap().domain, Domain::Both);
    assert_eq!(BuiltinTable::find("random").unwrap().domain, Domain::Host);
    assert_eq!(
        BuiltinTable::find("material_shader")
            .unwrap()
            .signature
            .result,
        BuiltinType::Opaque(polygl_hir::OpaqueType::Material)
    );
    let fill = BuiltinTable::find("fill").unwrap();
    assert_eq!(fill.signature.params[3].name, "a");
    assert!(fill.signature.params[3].default.is_some());
    assert_eq!(
        BuiltinTable::find("floor").unwrap().signature.result,
        BuiltinType::Int
    );
}

#[test]
fn public_parameter_names_match_the_design_contract() {
    let names = |builtin: &str| {
        BuiltinTable::find(builtin)
            .unwrap()
            .signature
            .params
            .iter()
            .map(|param| param.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(names("size"), ["w", "h"]);
    assert_eq!(names("rect"), ["x", "y", "w", "h"]);
    assert_eq!(names("circle"), ["x", "y", "r"]);
    assert_eq!(names("text"), ["s", "x", "y"]);
    assert_eq!(names("random"), ["a", "b"]);
    assert_eq!(names("material_shader"), ["name"]);
}

#[test]
fn event_schema_matches_the_common_core_contract() {
    let event = BuiltinTable::find_struct("Event").expect("Event is a builtin struct");
    let fields = event
        .fields
        .iter()
        .map(|field| (field.name, field.ty))
        .collect::<Vec<_>>();
    assert_eq!(
        fields,
        [
            ("kind", crate::BuiltinValueType::Scalar(BuiltinType::Str)),
            ("x", crate::BuiltinValueType::Scalar(BuiltinType::Float)),
            ("y", crate::BuiltinValueType::Scalar(BuiltinType::Float)),
            ("key", crate::BuiltinValueType::Option(BuiltinType::Str)),
        ]
    );
}
