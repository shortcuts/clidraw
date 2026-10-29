use serde_json::{json, Value};

const SHAPE_KINDS: &[&str] = &[
    "Rect",
    "Diamond",
    "Cylinder",
    "Parallelogram",
    "Hexagon",
    "Stadium",
    "Frame",
];
const LINE_KINDS: &[&str] = &["Line", "Arrow", "ArrowStart", "ArrowBoth"];
const STROKES: &[&str] = &["solid", "dashed", "dotted"];
const BORDERS: &[&str] = &["single", "rounded", "double", "heavy"];
const ALIGNS: &[&str] = &["left", "center", "right"];
const ALIGN_MODES: &[&str] = &[
    "left",
    "center",
    "right",
    "top",
    "middle",
    "bottom",
    "distribute_h",
    "distribute_v",
];
const BOOLS: &[&str] = &["true", "false"];
const Z_ORDERS: &[&str] = &["front", "back", "forward", "backward"];
const DIRS: &[&str] = &["td", "lr"];

/// One field of an op, as the reference page shows it.
pub struct FieldDoc {
    pub name: &'static str,
    pub required: bool,
    /// A JSON literal the parser accepts for this field.
    pub example: &'static str,
    /// Accepted values: a bare word is a string, `true` and `false` are booleans. Empty when the field takes any value.
    pub values: &'static [&'static str],
    pub note: &'static str,
}

/// One edit op. Add each new `Op` field here too: the test cannot see an optional field the table lacks.
pub struct OpDoc {
    pub name: &'static str,
    pub summary: &'static str,
    pub fields: &'static [FieldDoc],
}

pub const OPS: [OpDoc; 13] = [
    OpDoc {
        name: "add_shape",
        summary: "Adds a shape. Its size fits the label unless w and h are set.",
        fields: &[
            FieldDoc {
                name: "x",
                required: true,
                example: "0",
                values: &[],
                note: "Column, in cells.",
            },
            FieldDoc {
                name: "y",
                required: true,
                example: "0",
                values: &[],
                note: "Row, in cells.",
            },
            FieldDoc {
                name: "id",
                required: false,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "kind",
                required: false,
                example: "\"Rect\"",
                values: SHAPE_KINDS,
                note: "Default: Rect.",
            },
            FieldDoc {
                name: "w",
                required: false,
                example: "10",
                values: &[],
                note: "Width, in cells. Default fits the label.",
            },
            FieldDoc {
                name: "h",
                required: false,
                example: "3",
                values: &[],
                note: "Height, in cells. Default fits the label.",
            },
            FieldDoc {
                name: "label",
                required: false,
                example: "\"Hi\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "color",
                required: false,
                example: "\"red\"",
                values: &[],
                note: "A color name (red, orange, yellow, green, blue, violet) or a #rrggbb hex.",
            },
            FieldDoc {
                name: "fill",
                required: false,
                example: "\"blue\"",
                values: &[],
                note: "A color name (red, orange, yellow, green, blue, violet) or a #rrggbb hex.",
            },
            FieldDoc {
                name: "meta",
                required: false,
                example: "{\"file\": \"src/a.rs\"}",
                values: &[],
                note: "Key to value map.",
            },
        ],
    },
    OpDoc {
        name: "add_text",
        summary: "Adds a line of text.",
        fields: &[
            FieldDoc {
                name: "x",
                required: true,
                example: "0",
                values: &[],
                note: "Column, in cells.",
            },
            FieldDoc {
                name: "y",
                required: true,
                example: "0",
                values: &[],
                note: "Row, in cells.",
            },
            FieldDoc {
                name: "text",
                required: true,
                example: "\"Hi\"",
                values: &[],
                note: "One line.",
            },
            FieldDoc {
                name: "id",
                required: false,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "color",
                required: false,
                example: "\"red\"",
                values: &[],
                note: "A color name (red, orange, yellow, green, blue, violet) or a #rrggbb hex.",
            },
            FieldDoc {
                name: "meta",
                required: false,
                example: "{\"file\": \"src/a.rs\"}",
                values: &[],
                note: "Key to value map.",
            },
        ],
    },
    OpDoc {
        name: "connect",
        summary: "Adds a line from one endpoint to another.",
        fields: &[
            FieldDoc {
                name: "from",
                required: true,
                example: "\"a\"",
                values: &[],
                note: "A shape id, or a [x, y] cell for a free end.",
            },
            FieldDoc {
                name: "to",
                required: true,
                example: "[0, 0]",
                values: &[],
                note: "A shape id, or a [x, y] cell for a free end.",
            },
            FieldDoc {
                name: "id",
                required: false,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "kind",
                required: false,
                example: "\"Arrow\"",
                values: LINE_KINDS,
                note: "Default: Arrow.",
            },
            FieldDoc {
                name: "label",
                required: false,
                example: "\"Yes\"",
                values: &[],
                note: "One line.",
            },
            FieldDoc {
                name: "stroke",
                required: false,
                example: "\"dotted\"",
                values: STROKES,
                note: "Default: solid.",
            },
            FieldDoc {
                name: "meta",
                required: false,
                example: "{\"file\": \"src/a.rs\"}",
                values: &[],
                note: "Key to value map.",
            },
        ],
    },
    OpDoc {
        name: "set",
        summary: "Changes one element. Set at least one field besides id.",
        fields: &[
            FieldDoc {
                name: "id",
                required: true,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "label",
                required: false,
                example: "\"Hi\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "color",
                required: false,
                example: "\"red\"",
                values: &[],
                note: "A color name (red, orange, yellow, green, blue, violet) or a #rrggbb hex. Use none to clear the color.",
            },
            FieldDoc {
                name: "fill",
                required: false,
                example: "\"blue\"",
                values: &[],
                note: "A color name (red, orange, yellow, green, blue, violet) or a #rrggbb hex. Use none to clear the fill.",
            },
            FieldDoc {
                name: "stroke",
                required: false,
                example: "\"dashed\"",
                values: STROKES,
                note: "",
            },
            FieldDoc {
                name: "border",
                required: false,
                example: "\"rounded\"",
                values: BORDERS,
                note: "",
            },
            FieldDoc {
                name: "align",
                required: false,
                example: "\"left\"",
                values: ALIGNS,
                note: "",
            },
            FieldDoc {
                name: "bold",
                required: false,
                example: "true",
                values: BOOLS,
                note: "",
            },
            FieldDoc {
                name: "link",
                required: false,
                example: "\"https://example.com\"",
                values: &[],
                note: "One line. An empty string clears the link.",
            },
            FieldDoc {
                name: "kind",
                required: false,
                example: "\"Diamond\"",
                values: SHAPE_KINDS,
                note: "Changes the kind of a shape.",
            },
            FieldDoc {
                name: "head",
                required: false,
                example: "\"ArrowBoth\"",
                values: LINE_KINDS,
                note: "Changes the kind of a line.",
            },
            FieldDoc {
                name: "meta",
                required: false,
                example: "{\"file\": \"src/a.rs\"}",
                values: &[],
                note: "Key to value map. An empty value removes the key.",
            },
        ],
    },
    OpDoc {
        name: "resize",
        summary: "Sets the size of a shape.",
        fields: &[
            FieldDoc {
                name: "id",
                required: true,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "w",
                required: true,
                example: "10",
                values: &[],
                note: "Width, in cells.",
            },
            FieldDoc {
                name: "h",
                required: true,
                example: "3",
                values: &[],
                note: "Height, in cells.",
            },
        ],
    },
    OpDoc {
        name: "move",
        summary: "Moves one element.",
        fields: &[
            FieldDoc {
                name: "id",
                required: true,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "dx",
                required: true,
                example: "1",
                values: &[],
                note: "Columns to move. Negative moves left.",
            },
            FieldDoc {
                name: "dy",
                required: true,
                example: "0",
                values: &[],
                note: "Rows to move. Negative moves up.",
            },
        ],
    },
    OpDoc {
        name: "duplicate",
        summary: "Copies elements and the lines between them, shifted by dx and dy. Returns the new ids.",
        fields: &[
            FieldDoc {
                name: "ids",
                required: true,
                example: "[\"a\", \"b\"]",
                values: &[],
                note: "Elements to copy. A line is copied when it is listed or both its ends are listed.",
            },
            FieldDoc {
                name: "dx",
                required: true,
                example: "1",
                values: &[],
                note: "Columns to shift the copy. Negative shifts left.",
            },
            FieldDoc {
                name: "dy",
                required: true,
                example: "0",
                values: &[],
                note: "Rows to shift the copy. Negative shifts up.",
            },
        ],
    },
    OpDoc {
        name: "delete",
        summary: "Deletes one element.",
        fields: &[FieldDoc {
            name: "id",
            required: true,
            example: "\"a\"",
            values: &[],
            note: "",
        }],
    },
    OpDoc {
        name: "group",
        summary: "Groups two or more elements.",
        fields: &[FieldDoc {
            name: "ids",
            required: true,
            example: "[\"a\", \"b\"]",
            values: &[],
            note: "At least two ids.",
        }],
    },
    OpDoc {
        name: "ungroup",
        summary: "Removes the group that holds the elements.",
        fields: &[FieldDoc {
            name: "ids",
            required: true,
            example: "[\"a\", \"b\"]",
            values: &[],
            note: "",
        }],
    },
    OpDoc {
        name: "order",
        summary: "Changes the stacking order of elements.",
        fields: &[
            FieldDoc {
                name: "ids",
                required: true,
                example: "[\"a\", \"b\"]",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "z",
                required: true,
                example: "\"front\"",
                values: Z_ORDERS,
                note: "",
            },
        ],
    },
    OpDoc {
        name: "layout",
        summary: "Arranges elements by the lines between them.",
        fields: &[
            FieldDoc {
                name: "ids",
                required: false,
                example: "[\"a\", \"b\"]",
                values: &[],
                note: "Without ids, arranges every element.",
            },
            FieldDoc {
                name: "dir",
                required: false,
                example: "\"td\"",
                values: DIRS,
                note: "td runs top-down, lr runs left-right. Default: td.",
            },
        ],
    },
    OpDoc {
        name: "align",
        summary: "Aligns or spreads elements. Lines do not move.",
        fields: &[
            FieldDoc {
                name: "ids",
                required: true,
                example: "[\"a\", \"b\"]",
                values: &[],
                note: "At least two ids. The distribute modes need three.",
            },
            FieldDoc {
                name: "mode",
                required: true,
                example: "\"left\"",
                values: ALIGN_MODES,
                note: "",
            },
        ],
    },
];

fn field_schema(op: &str, f: &FieldDoc) -> Value {
    let mut prop = match (op, f.name) {
        // An end is a shape id or an [x, y] cell, which one example cannot show.
        ("connect", "from" | "to") => {
            json!({"type": ["string", "array"], "items": {"type": "integer"}})
        }
        _ => match serde_json::from_str(f.example) {
            Ok(Value::Number(_)) => json!({"type": "integer"}),
            Ok(Value::Bool(_)) => json!({"type": "boolean"}),
            Ok(Value::Array(_)) => json!({"type": "array", "items": {"type": "string"}}),
            Ok(Value::Object(_)) => {
                json!({"type": "object", "additionalProperties": {"type": "string"}})
            }
            _ => json!({"type": "string"}),
        },
    };
    if prop["type"] == "string" && !f.values.is_empty() {
        prop["enum"] = json!(f.values);
    }
    if !f.note.is_empty() {
        prop["description"] = json!(f.note);
    }
    prop
}

/// One JSON Schema object per op, so a client sees each op's own fields and values.
pub fn op_items_schema() -> Value {
    let ops: Vec<Value> = OPS
        .iter()
        .map(|op| {
            let mut props = serde_json::Map::new();
            props.insert("op".into(), json!({"const": op.name}));
            for f in op.fields {
                props.insert(f.name.into(), field_schema(op.name, f));
            }
            let required: Vec<&str> = std::iter::once("op")
                .chain(op.fields.iter().filter(|f| f.required).map(|f| f.name))
                .collect();
            json!({
                "type": "object",
                "description": op.summary,
                "properties": props,
                "required": required,
                "additionalProperties": false,
            })
        })
        .collect();
    json!({"type": "object", "oneOf": ops})
}

/// One entry per op for the MCP `edit` description: `name (required, optional a, b): summary`.
pub fn op_list() -> String {
    let names = |op: &OpDoc, required: bool| {
        let v: Vec<&str> = op
            .fields
            .iter()
            .filter(|f| f.required == required)
            .map(|f| f.name)
            .collect();
        v.join(", ")
    };
    OPS.iter()
        .map(|op| {
            let (req, opt) = (names(op, true), names(op, false));
            let args = match (req.is_empty(), opt.is_empty()) {
                (_, true) => req,
                (true, false) => format!("optional {opt}"),
                (false, false) => format!("{req}, optional {opt}"),
            };
            format!("{} ({args}): {}", op.name, op.summary)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_ends_take_an_id_or_a_cell() {
        let to = OPS[2].fields.iter().find(|f| f.name == "to").unwrap();
        assert_eq!(
            field_schema("connect", to)["type"],
            json!(["string", "array"])
        );
    }

    #[test]
    fn op_list_names_required_and_optional_fields() {
        let list = op_list();
        assert!(list.contains("resize (id, w, h): "), "{list}");
        assert!(list.contains("layout (optional ids, dir): "), "{list}");
        assert!(
            list.contains("add_text (x, y, text, optional id, color, meta): "),
            "{list}"
        );
    }
}
