//! Conversión de tipos `lsp-types` a los DTO del frontend. Posiciones UTF-16 (igual que CodeMirror).

use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit, Diagnostic, DiagnosticSeverity,
    Documentation, GotoDefinitionResponse, Hover, HoverContents, InsertTextFormat, Location, MarkedString, Range,
    Url,
};
use serde_json::Value;

use super::{CompletionDto, DiagSeverity, DiagnosticDto, HoverDto, LocationDto, TextEditDto};

const MAX_COMPLETIONS: usize = 200;

pub fn file_path_of(uri: &Url) -> Option<String> {
    uri.to_file_path().ok().map(|p| p.to_string_lossy().into_owned())
}

pub fn diagnostic_dto(d: Diagnostic) -> DiagnosticDto {
    let severity = match d.severity {
        Some(DiagnosticSeverity::WARNING) => DiagSeverity::Warning,
        Some(DiagnosticSeverity::INFORMATION) => DiagSeverity::Info,
        Some(DiagnosticSeverity::HINT) => DiagSeverity::Hint,
        _ => DiagSeverity::Error,
    };
    DiagnosticDto {
        from_line: d.range.start.line,
        from_char: d.range.start.character,
        to_line: d.range.end.line,
        to_char: d.range.end.character,
        severity,
        message: d.message,
        source: d.source,
    }
}

fn kind_name(k: Option<CompletionItemKind>) -> Option<String> {
    let k = k?;
    let name = if k == CompletionItemKind::FUNCTION || k == CompletionItemKind::CONSTRUCTOR {
        "function"
    } else if k == CompletionItemKind::METHOD {
        "method"
    } else if k == CompletionItemKind::VARIABLE {
        "variable"
    } else if k == CompletionItemKind::CLASS || k == CompletionItemKind::STRUCT {
        "class"
    } else if k == CompletionItemKind::INTERFACE {
        "interface"
    } else if k == CompletionItemKind::KEYWORD {
        "keyword"
    } else if k == CompletionItemKind::PROPERTY || k == CompletionItemKind::FIELD {
        "property"
    } else if k == CompletionItemKind::MODULE {
        "namespace"
    } else if k == CompletionItemKind::CONSTANT {
        "constant"
    } else if k == CompletionItemKind::ENUM || k == CompletionItemKind::ENUM_MEMBER {
        "enum"
    } else if k == CompletionItemKind::SNIPPET {
        "snippet"
    } else {
        "text"
    };
    Some(name.to_string())
}

fn edit_dto(range: Range, new_text: String) -> TextEditDto {
    TextEditDto {
        from_line: range.start.line,
        from_char: range.start.character,
        to_line: range.end.line,
        to_char: range.end.character,
        new_text,
    }
}

fn completion_dto(item: CompletionItem) -> CompletionDto {
    let text_edit = match item.text_edit {
        Some(CompletionTextEdit::Edit(e)) => Some(edit_dto(e.range, e.new_text)),
        Some(CompletionTextEdit::InsertAndReplace(e)) => Some(edit_dto(e.replace, e.new_text)),
        None => None,
    };
    let insert_text = text_edit
        .as_ref()
        .map(|e| e.new_text.clone())
        .or(item.insert_text)
        .unwrap_or_else(|| item.label.clone());
    let documentation = item.documentation.map(|d| match d {
        Documentation::String(s) => s,
        Documentation::MarkupContent(m) => m.value,
    });
    CompletionDto {
        kind: kind_name(item.kind),
        detail: item.detail,
        documentation,
        insert_text,
        is_snippet: item.insert_text_format == Some(InsertTextFormat::SNIPPET),
        sort_text: item.sort_text,
        filter_text: item.filter_text,
        text_edit,
        label: item.label,
    }
}

/// `textDocument/completion` (array, `CompletionList` o `null`) → DTOs (máx. 200).
pub fn completion_from(v: Value) -> Vec<CompletionDto> {
    let items = match serde_json::from_value::<Option<CompletionResponse>>(v) {
        Ok(Some(CompletionResponse::Array(a))) => a,
        Ok(Some(CompletionResponse::List(l))) => l.items,
        _ => return Vec::new(),
    };
    items.into_iter().take(MAX_COMPLETIONS).map(completion_dto).collect()
}

fn marked(m: MarkedString) -> String {
    match m {
        MarkedString::String(s) => s,
        MarkedString::LanguageString(ls) => format!("```{}\n{}\n```", ls.language, ls.value),
    }
}

/// `textDocument/hover` → markdown.
pub fn hover_from(v: Value) -> Option<HoverDto> {
    let h = serde_json::from_value::<Option<Hover>>(v).ok()??;
    let markdown = match h.contents {
        HoverContents::Scalar(m) => marked(m),
        HoverContents::Array(a) => a.into_iter().map(marked).collect::<Vec<_>>().join("\n\n"),
        HoverContents::Markup(m) => m.value,
    };
    if markdown.trim().is_empty() {
        None
    } else {
        Some(HoverDto { markdown })
    }
}

fn location_dto(uri: &Url, range: Range) -> Option<LocationDto> {
    Some(LocationDto {
        path: file_path_of(uri)?,
        line: range.start.line,
        character: range.start.character,
        end_line: range.end.line,
        end_character: range.end.character,
    })
}

/// `textDocument/definition` (Location | Location[] | LocationLink[] | null) → ubicaciones en disco.
pub fn locations_from(v: Value) -> Vec<LocationDto> {
    match serde_json::from_value::<Option<GotoDefinitionResponse>>(v) {
        Ok(Some(GotoDefinitionResponse::Scalar(l))) => location_dto(&l.uri, l.range).into_iter().collect(),
        Ok(Some(GotoDefinitionResponse::Array(a))) => {
            a.iter().filter_map(|l: &Location| location_dto(&l.uri, l.range)).collect()
        }
        Ok(Some(GotoDefinitionResponse::Link(links))) => links
            .iter()
            .filter_map(|l| location_dto(&l.target_uri, l.target_selection_range))
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn completion_list_with_snippet_and_text_edit() {
        let v = json!({"isIncomplete": false, "items": [
            {"label": "push", "kind": 2, "detail": "fn(x)", "insertText": "push($0)", "insertTextFormat": 2,
             "documentation": {"kind": "markdown", "value": "docs"}},
            {"label": "len", "kind": 3, "textEdit": {"range": {"start": {"line": 1, "character": 2}, "end": {"line": 1, "character": 4}}, "newText": "len()"}},
            {"label": "plain"}
        ]});
        let r = completion_from(v);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].kind.as_deref(), Some("method"));
        assert!(r[0].is_snippet);
        assert_eq!(r[0].insert_text, "push($0)");
        assert_eq!(r[0].documentation.as_deref(), Some("docs"));
        assert_eq!(r[1].kind.as_deref(), Some("function"));
        assert_eq!(r[1].insert_text, "len()");
        assert_eq!(r[1].text_edit.as_ref().unwrap().from_char, 2);
        assert_eq!(r[2].insert_text, "plain");
        assert!(r[2].text_edit.is_none());
        assert!(completion_from(Value::Null).is_empty());
    }

    #[test]
    fn completion_is_capped() {
        let items: Vec<Value> = (0..500).map(|i| json!({"label": format!("a{i}")})).collect();
        assert_eq!(completion_from(Value::Array(items)).len(), MAX_COMPLETIONS);
    }

    #[test]
    fn hover_variants() {
        let m = hover_from(json!({"contents": {"kind": "markdown", "value": "```py\nx: int\n```"}})).unwrap();
        assert!(m.markdown.contains("x: int"));
        let a = hover_from(json!({"contents": [{"language": "rust", "value": "fn f()"}, "texto"]})).unwrap();
        assert_eq!(a.markdown, "```rust\nfn f()\n```\n\ntexto");
        assert!(hover_from(Value::Null).is_none());
        assert!(hover_from(json!({"contents": ""})).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn definition_variants() {
        let rng = json!({"start": {"line": 3, "character": 4}, "end": {"line": 3, "character": 9}});
        let one = locations_from(json!({"uri": "file:///tmp/a.py", "range": rng}));
        assert_eq!(one[0].path, "/tmp/a.py");
        assert_eq!((one[0].line, one[0].character, one[0].end_character), (3, 4, 9));
        let link =
            locations_from(json!([{"targetUri": "file:///tmp/b.rs", "targetRange": rng, "targetSelectionRange": rng}]));
        assert_eq!(link[0].path, "/tmp/b.rs");
        assert!(locations_from(Value::Null).is_empty());
        assert!(locations_from(json!({"uri": "http://x/y", "range": rng})).is_empty());
    }

    #[test]
    fn diagnostic_severity_mapping() {
        let d = |s: Option<DiagnosticSeverity>| {
            diagnostic_dto(Diagnostic { severity: s, message: "m".into(), ..Default::default() }).severity
        };
        assert_eq!(d(None), DiagSeverity::Error);
        assert_eq!(d(Some(DiagnosticSeverity::WARNING)), DiagSeverity::Warning);
        assert_eq!(d(Some(DiagnosticSeverity::INFORMATION)), DiagSeverity::Info);
        assert_eq!(d(Some(DiagnosticSeverity::HINT)), DiagSeverity::Hint);
    }
}
