use polygl_adapter_api::{LanguageAdapter, LowerCtx};
use polygl_adapter_ruby::RubyAdapter;
use polygl_core::BuiltinTable;
use polygl_hir::Symbol;
use polygl_lir::{
    BinaryOp, Block, CallTarget, Constant, Domain, EntryKind, EntryPoint, Expr, ExprKind, Field,
    FieldInit, Literal, Module, Parameter, Range, Statement, StatementKind, StructDef,
};
use polygl_span::{SourceFile, SourceId, Span};
use polygl_types::Type;

use crate::{EmitError, GlslBackend, UniformSource};

static SHADER_FILE_ID: AtomicUsize = AtomicUsize::new(0);

fn compile(source: &str) -> crate::GlslArtifacts {
    let source = SourceFile::new(SourceId::new(1), "shader.rb", source);
    let hir = RubyAdapter
        .lower(&source, &mut LowerCtx::new(&BuiltinTable))
        .expect("Ruby shader should lower");
    let typed = polygl_types::analyze(&hir).expect("Ruby shader should type-check");
    let lir = polygl_lir::lower(&typed);
    let split = polygl_lir::split(&lir).expect("Ruby shader should pass GPU validation");
    GlslBackend::new()
        .generate(&split.gpu)
        .expect("validated GPU LIR should emit")
}

#[test]
fn emits_glsl_es_300_from_ruby_shader_entries() {
    let artifacts = compile(
        r#"
# @pgl position: vec3
def vertex_plasma(position)
  vec4(0.0, 0.0, 0.0, 1.0)
end

def fragment_plasma
  rounded = round(-1.5)
  phase = time()
  vec4(phase, 0.25, 0.5, 1.0)
end
"#,
    );
    assert_eq!(artifacts.shaders.len(), 1);
    let shader = &artifacts.shaders[0];
    assert_eq!(shader.name, "plasma");
    assert!(shader.vertex.starts_with("#version 300 es\n"));
    assert!(
        shader
            .vertex
            .contains("layout(location = 0) in vec3 a_position;")
    );
    assert!(shader.vertex.contains("gl_Position = vec4("));
    assert!(shader.fragment.contains("uniform float u_time;"));
    assert!(shader.fragment.contains("ceil("));
    assert!(shader.fragment.contains("floor("));
    assert!(shader.fragment.contains("out vec4 out_color;"));
    assert!(shader.fragment.contains("out_color = vec4("));
    assert_eq!(shader.attributes[0].name, "position");
    assert_eq!(shader.attributes[0].location, 0);
    assert_eq!(shader.uniforms[0].name, "u_time");
    validate_with_glslang(&shader.vertex, "vert");
    validate_with_glslang(&shader.fragment, "frag");
}

#[test]
fn reflects_transforms_and_user_textures_from_shader_uniforms() {
    let artifacts = compile(
        r#"
# @pgl position: vec3
def vertex_textured(position)
  u_proj * u_view * u_model * vec4(position, 1.0)
end

# @pgl texture_map: Texture
def fragment_textured
  sample(texture_map, vec2(0.5, 0.5))
end
"#,
    );
    let shader = &artifacts.shaders[0];
    assert!(shader.vertex.contains("uniform mat4 u_model;"));
    assert!(shader.vertex.contains("uniform mat4 u_view;"));
    assert!(shader.vertex.contains("uniform mat4 u_proj;"));
    assert!(shader.vertex.contains("u_proj * u_view"));
    assert!(shader.vertex.contains("vec4(a_position, 1.0)"));
    assert!(shader.fragment.contains("uniform sampler2D pgl_u_"));
    assert!(shader.fragment.contains("texture(pgl_u_"));
    assert_eq!(
        shader
            .uniforms
            .iter()
            .filter(|uniform| uniform.source == UniformSource::Automatic)
            .map(|uniform| uniform.name.as_str())
            .collect::<Vec<_>>(),
        ["u_model", "u_proj", "u_view"]
    );
    let texture = shader
        .uniforms
        .iter()
        .find(|uniform| uniform.name == "texture_map")
        .expect("user texture reflection");
    assert_eq!(texture.ty, Type::Opaque(polygl_hir::OpaqueType::Texture));
    assert_eq!(texture.source, UniformSource::User);
    validate_with_glslang(&shader.vertex, "vert");
    validate_with_glslang(&shader.fragment, "frag");
}

#[test]
fn reflects_only_declarations_reachable_from_each_shader_pair() {
    let artifacts = compile(
        r#"
def vertex_animated
  vec4(0.0, 0.0, 0.0, 1.0)
end

def fragment_animated
  vec4(time(), 0.0, 0.0, 1.0)
end

def vertex_static
  vec4(0.0, 0.0, 0.0, 1.0)
end

def fragment_static
  vec4(0.0, 1.0, 0.0, 1.0)
end
"#,
    );
    assert_eq!(artifacts.shaders.len(), 2);
    let animated = artifacts
        .shaders
        .iter()
        .find(|shader| shader.name == "animated")
        .expect("animated pair");
    let static_shader = artifacts
        .shaders
        .iter()
        .find(|shader| shader.name == "static")
        .expect("static pair");
    assert_eq!(animated.uniforms.len(), 1);
    assert!(animated.fragment.contains("uniform float u_time;"));
    assert!(static_shader.uniforms.is_empty());
    assert!(!static_shader.vertex.contains("u_time"));
    assert!(!static_shader.fragment.contains("u_time"));
}

#[test]
fn preserves_structured_control_flow_and_floor_integer_semantics() {
    let artifacts = compile(
        r#"
def helper
  value = -3 / 2
  while value < 0
    value = value + 1
  end
  value
end

def vertex_flow
  helper()
  vec4(0.0, 0.0, 0.0, 1.0)
end

def fragment_flow
  vec4(1.0, 1.0, 1.0, 1.0)
end
"#,
    );
    let shader = &artifacts.shaders[0];
    assert!(shader.vertex.contains("pgl_int_div(-3, 2)"));
    assert!(shader.vertex.contains("if (right == 0) return 0;"));
    assert!(
        shader
            .vertex
            .contains("left == (-2147483647 - 1) && right == -1")
    );
    assert!(shader.vertex.contains("while ("));
    assert!(shader.vertex.contains(" + 1);"));
}

#[test]
fn emits_safe_ranges_dynamic_constants_and_flat_integer_varyings() {
    let span = test_span();
    let time = BuiltinTable::find("time").expect("registered time builtin");
    let phase = Constant {
        name: "PHASE".to_owned(),
        ty: Type::Float,
        value: Expr::new(
            ExprKind::Call {
                target: CallTarget::Runtime(time.runtime_op),
                args: Vec::new(),
            },
            Type::Float,
            span,
        ),
        domain: Domain::Gpu,
        span,
    };
    let varying_type = Type::Struct(Symbol::new("Varyings"));
    let vertex_result = Expr::new(
        ExprKind::Struct {
            name: "Varyings".to_owned(),
            fields: vec![
                FieldInit {
                    name: "clip_pos".to_owned(),
                    value: vec4(span, [0.0, 0.0, 0.0, 1.0]),
                    span,
                },
                FieldInit {
                    name: "index".to_owned(),
                    value: Expr::new(ExprKind::Literal(Literal::Int(7)), Type::Int, span),
                    span,
                },
            ],
        },
        varying_type.clone(),
        span,
    );
    let vertex = EntryPoint {
        kind: EntryKind::Vertex("manual".to_owned()),
        params: Vec::new(),
        result: varying_type.clone(),
        body: Block {
            statements: vec![
                Statement::new(
                    StatementKind::For {
                        variable: "i".to_owned(),
                        range: Range {
                            start: Expr::new(
                                ExprKind::Literal(Literal::Int(i32::MAX)),
                                Type::Int,
                                span,
                            ),
                            end: Expr::new(
                                ExprKind::Literal(Literal::Int(i32::MAX)),
                                Type::Int,
                                span,
                            ),
                            inclusive: true,
                            span,
                        },
                        body: Block {
                            statements: Vec::new(),
                            span,
                        },
                    },
                    span,
                ),
                Statement::new(
                    StatementKind::Expr(Expr::new(
                        ExprKind::Binary {
                            op: BinaryOp::TruncatingRemainder,
                            left: Box::new(Expr::new(
                                ExprKind::Binary {
                                    op: BinaryOp::Subtract,
                                    left: Box::new(Expr::new(
                                        ExprKind::Literal(Literal::Int(-2_147_483_647)),
                                        Type::Int,
                                        span,
                                    )),
                                    right: Box::new(Expr::new(
                                        ExprKind::Literal(Literal::Int(1)),
                                        Type::Int,
                                        span,
                                    )),
                                },
                                Type::Int,
                                span,
                            )),
                            right: Box::new(Expr::new(
                                ExprKind::Literal(Literal::Int(-1)),
                                Type::Int,
                                span,
                            )),
                        },
                        Type::Int,
                        span,
                    )),
                    span,
                ),
                Statement::new(StatementKind::Return(Some(vertex_result)), span),
            ],
            span,
        },
        domain: Domain::Gpu,
        span,
    };
    let fragment = EntryPoint {
        kind: EntryKind::Fragment("manual".to_owned()),
        params: vec![Parameter {
            name: "varyings".to_owned(),
            ty: varying_type,
            span,
        }],
        result: Type::Vector(4),
        body: Block {
            statements: vec![Statement::new(
                StatementKind::Return(Some(Expr::new(
                    ExprKind::Vector {
                        size: 4,
                        args: vec![
                            Expr::new(ExprKind::Constant("PHASE".to_owned()), Type::Float, span),
                            float(span, 0.0),
                            float(span, 0.0),
                            float(span, 1.0),
                        ],
                    },
                    Type::Vector(4),
                    span,
                ))),
                span,
            )],
            span,
        },
        domain: Domain::Gpu,
        span,
    };
    let module = Module {
        functions: Vec::new(),
        structs: vec![StructDef {
            name: "Varyings".to_owned(),
            fields: vec![
                Field {
                    name: "clip_pos".to_owned(),
                    ty: Type::Vector(4),
                    span,
                },
                Field {
                    name: "index".to_owned(),
                    ty: Type::Int,
                    span,
                },
            ],
            span,
        }],
        constants: vec![phase],
        entries: vec![vertex, fragment],
        span,
    };
    let split = polygl_lir::split(&module).expect("manual module should satisfy the GPU ABI");
    let artifacts = GlslBackend::new()
        .generate(&split.gpu)
        .expect("manual GPU module should emit");
    let shader = &artifacts.shaders[0];
    assert!(shader.vertex.contains("flat out int"));
    assert!(shader.fragment.contains("flat in int"));
    assert!(shader.fragment.contains("#define pgl_c_"));
    assert!(shader.fragment.contains("(u_time)"));
    assert_eq!(shader.vertex.matches("= 2147483647;").count(), 2);
    assert!(shader.vertex.contains("bool pgl_l_"));
    assert!(shader.vertex.contains("? 0 : 1"));
    assert!(
        shader
            .vertex
            .contains("pgl_trunc_mod((-2147483647 - 1), -1);")
    );
    assert!(!shader.vertex.contains("__"));
    assert!(!shader.fragment.contains("__"));
    validate_with_glslang(&shader.vertex, "vert");
    validate_with_glslang(&shader.fragment, "frag");

    let mut malformed = module.clone();
    malformed.entries[0].params.push(Parameter {
        name: "unknown".to_owned(),
        ty: Type::Vector(3),
        span,
    });
    assert!(matches!(
        GlslBackend::new().generate(&malformed),
        Err(EmitError::InvalidAttribute(name)) if name == "unknown"
    ));

    let mut empty_varying = module;
    empty_varying.structs[0].fields.clear();
    assert!(matches!(
        GlslBackend::new().generate(&empty_varying),
        Err(EmitError::InvalidStageResult { .. })
    ));
}

fn validate_with_glslang(source: &str, stage: &str) {
    if Command::new("glslangValidator")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let path = std::env::temp_dir().join(format!(
        "polygl-glsl-{}-{}-{stage}.{stage}",
        std::process::id(),
        SHADER_FILE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, source).expect("temporary shader source should be writable");
    let output = Command::new("glslangValidator")
        .args(["-S", stage])
        .arg(&path)
        .output()
        .expect("glslangValidator should run");
    let _ = fs::remove_file(&path);
    assert!(
        output.status.success(),
        "generated {stage} shader failed validation:\n{}{}\n--- source ---\n{source}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn test_span() -> Span {
    SourceFile::new(SourceId::new(9), "manual.rb", "x")
        .span(0, 1)
        .unwrap()
}

fn float(span: Span, value: f64) -> Expr {
    Expr::new(ExprKind::Literal(Literal::Float(value)), Type::Float, span)
}

fn vec4(span: Span, values: [f64; 4]) -> Expr {
    Expr::new(
        ExprKind::Vector {
            size: 4,
            args: values.into_iter().map(|value| float(span, value)).collect(),
        },
        Type::Vector(4),
        span,
    )
}
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
