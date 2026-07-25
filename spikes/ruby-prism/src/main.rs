use ruby_prism::{Node, parse};

fn line_columns(source: &[u8], offset: usize) -> (usize, usize, usize, usize) {
    let before = &source[..offset];
    let line = before.iter().filter(|&&byte| byte == b'\n').count() + 1;
    let line_start = before
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |index| index + 1);
    let line_prefix = std::str::from_utf8(&source[line_start..offset]).expect("UTF-8 source");
    (
        line,
        offset - line_start + 1,
        line_prefix.chars().count() + 1,
        line_prefix.encode_utf16().count() + 1,
    )
}

fn node_kind(node: &Node<'_>) -> &'static str {
    if node.as_integer_node().is_some() {
        "IntegerNode"
    } else if node.as_float_node().is_some() {
        "FloatNode"
    } else if node.as_string_node().is_some() {
        "StringNode"
    } else if node.as_interpolated_string_node().is_some() {
        "InterpolatedStringNode"
    } else if node.as_symbol_node().is_some() {
        "SymbolNode"
    } else if node.as_true_node().is_some() {
        "TrueNode"
    } else if node.as_false_node().is_some() {
        "FalseNode"
    } else if node.as_nil_node().is_some() {
        "NilNode"
    } else if node.as_array_node().is_some() {
        "ArrayNode"
    } else if node.as_hash_node().is_some() {
        "HashNode"
    } else {
        "other"
    }
}

fn print_node(source: &[u8], node: &Node<'_>, prefix: &str) {
    let location = node.location();
    let (line, byte_column, scalar_column, utf16_column) =
        line_columns(source, location.start_offset());
    println!(
        "{}{} {}..{} line={} byte_col={} scalar_col={} utf16_col={} raw={:?}",
        prefix,
        node_kind(node),
        location.start_offset(),
        location.end_offset(),
        line,
        byte_column,
        scalar_column,
        utf16_column,
        String::from_utf8_lossy(location.as_slice())
    );
}

fn main() {
    let source = concat!(
        "# 日本語\r\n",
        "# @pgl x: float\r\n",
        "42\r\n",
        "1_000\r\n",
        "3.25e1\r\n",
        "\"héllo\\n\"\r\n",
        "\"v=#{x}\"\r\n",
        ":name\r\n",
        "true\r\n",
        "false\r\n",
        "nil\r\n",
        "[\"日\", 42]\r\n",
        "{a: 1}\r\n",
        "=begin\r\n",
        "@pgl y: int\r\n",
        "=end\r\n",
    );
    let result = parse(source.as_bytes());
    assert_eq!(result.errors().count(), 0);

    let statements = result
        .node()
        .as_program_node()
        .expect("program")
        .statements();
    let mut observed_kinds = Vec::new();
    for node in statements.body().iter() {
        print_node(source.as_bytes(), &node, "");
        observed_kinds.push(node_kind(&node));
        if let Some(integer) = node.as_integer_node() {
            let value: i32 = integer.value().try_into().expect("i32 literal");
            println!("  integer_value={value}");
        }
        if let Some(float) = node.as_float_node() {
            println!("  float_value={:?}", float.value());
        }
        if let Some(string) = node.as_string_node() {
            println!(
                "  string_unescaped={:?}",
                String::from_utf8_lossy(string.unescaped())
            );
        }
        if let Some(array) = node.as_array_node() {
            for element in array.elements().iter() {
                print_node(source.as_bytes(), &element, "  element=");
            }
        }
    }

    let required_kinds = [
        "IntegerNode",
        "FloatNode",
        "StringNode",
        "InterpolatedStringNode",
        "SymbolNode",
        "TrueNode",
        "FalseNode",
        "NilNode",
        "ArrayNode",
        "HashNode",
    ];
    for required in required_kinds {
        assert!(observed_kinds.contains(&required), "missing {required}");
    }

    let comments = result.comments().collect::<Vec<_>>();
    assert_eq!(comments.len(), 3);
    assert!(
        comments
            .iter()
            .any(|comment| comment.text().starts_with(b"# @pgl x: float"))
    );
    for comment in comments {
        let location = comment.location();
        let (line, byte_column, scalar_column, utf16_column) =
            line_columns(source.as_bytes(), location.start_offset());
        println!(
            "{:?} {}..{} line={} byte_col={} scalar_col={} utf16_col={} raw={:?}",
            comment.type_(),
            location.start_offset(),
            location.end_offset(),
            line,
            byte_column,
            scalar_column,
            utf16_column,
            String::from_utf8_lossy(comment.text())
        );
    }
}
