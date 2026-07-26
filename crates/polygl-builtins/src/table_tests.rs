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
    assert_eq!(
        BuiltinTable::all()
            .iter()
            .filter(|builtin| builtin.tier == BuiltinTier::Tier2)
            .map(|builtin| builtin.name)
            .collect::<Vec<_>>(),
        [
            "material_shader",
            "mesh_box",
            "mesh_sphere",
            "mesh_plane",
            "mesh_from",
            "material_basic",
            "node_add",
            "node_set_pos",
            "node_set_rot",
            "node_set_scale",
            "camera_perspective",
            "camera_look_at",
            "light_directional",
            "texture_load",
            "shader_set",
            "sample",
        ]
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
    assert_eq!(names("mesh_from"), ["vertices", "indices"]);
    assert_eq!(names("node_add"), ["mesh", "material"]);
    assert_eq!(names("camera_look_at"), ["eye", "target", "up"]);
    assert_eq!(names("shader_set"), ["node", "name", "value"]);
}

#[test]
fn tier_two_signatures_preserve_handle_and_shader_value_types() {
    let mesh = BuiltinType::Opaque(polygl_hir::OpaqueType::Mesh);
    let node = BuiltinType::Opaque(polygl_hir::OpaqueType::Node);
    let texture = BuiltinType::Opaque(polygl_hir::OpaqueType::Texture);
    assert_eq!(
        BuiltinTable::find("mesh_box").unwrap().signature.result,
        mesh
    );
    assert_eq!(
        BuiltinTable::find("node_add").unwrap().signature.result,
        node
    );
    assert_eq!(
        BuiltinTable::find("texture_load").unwrap().signature.result,
        texture
    );
    assert_eq!(
        BuiltinTable::find("shader_set").unwrap().signature.params[2].ty,
        BuiltinType::ShaderValue
    );
    assert_eq!(BuiltinTable::find("sample").unwrap().domain, Domain::Gpu);
    assert_eq!(
        BuiltinTable::find("sample").unwrap().signature.result,
        BuiltinType::Vec4
    );
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
