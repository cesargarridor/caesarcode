//! Lectura tabular (csv/tsv en streaming; xlsx/xls/ods con calamine).
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use calamine::{open_workbook_auto, Reader};
use serde::Serialize;

use crate::error::{AppError, AppResult};

pub const DEFAULT_LIMIT: usize = 5000;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabularData {
    /// Hojas (vacío en csv/tsv).
    pub sheets: Vec<String>,
    /// Hoja devuelta (None en csv/tsv).
    pub sheet: Option<String>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Filas de datos reales (sin cabecera).
    pub total_rows: usize,
    pub file_size: u64,
    /// Solo csv/tsv: `,` `;` o tabulador.
    pub delimiter: Option<String>,
}

fn ext(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

pub fn read(path: &Path, sheet: Option<&str>, limit: usize) -> AppResult<TabularData> {
    let meta = std::fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AppError::NotFound(path.display().to_string()),
        _ => AppError::Io(e),
    })?;
    if !meta.is_file() {
        return Err(AppError::InvalidArgument(format!("no es un archivo: {}", path.display())));
    }
    let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
    match ext(path).as_str() {
        "csv" | "tsv" | "txt" => read_csv(path, limit, meta.len()),
        "xlsx" | "xlsm" | "xlsb" | "xls" | "ods" => read_workbook(path, sheet, limit, meta.len()),
        other => Err(AppError::InvalidArgument(format!("formato tabular no soportado: .{other}"))),
    }
}

/// `,` `;` o tabulador según la primera línea (.tsv fuerza tabulador).
fn detect_delimiter(path: &Path) -> AppResult<u8> {
    if ext(path) == "tsv" {
        return Ok(b'\t');
    }
    let mut line = Vec::new();
    // Máx. 64 KiB de la primera línea: basta para detectar el separador.
    BufReader::new(File::open(path)?.take(64 * 1024)).read_until(b'\n', &mut line)?;
    let count = |d: u8| line.iter().filter(|b| **b == d).count();
    let (c, s, t) = (count(b','), count(b';'), count(b'\t'));
    Ok(if t > c && t >= s {
        b'\t'
    } else if s > c {
        b';'
    } else {
        b','
    })
}

fn read_csv(path: &Path, limit: usize, file_size: u64) -> AppResult<TabularData> {
    let delim = detect_delimiter(path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(BufReader::new(File::open(path)?));
    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut total = 0usize;
    let mut first = true;
    let mut rec = csv::ByteRecord::new();
    loop {
        match rdr.read_byte_record(&mut rec) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) if e.is_io_error() => return Err(AppError::Other(format!("csv: {e}"))),
            // Fila mal formada: se salta sin abortar.
            Err(_) => continue,
        }
        let cells = rec.iter().map(|c| String::from_utf8_lossy(c).into_owned());
        if first {
            first = false;
            headers = cells.map(|c| c.trim_start_matches('\u{feff}').to_string()).collect();
            continue;
        }
        total += 1;
        if rows.len() < limit {
            rows.push(cells.collect());
        }
    }
    Ok(TabularData {
        sheets: Vec::new(),
        sheet: None,
        headers,
        rows,
        total_rows: total,
        file_size,
        delimiter: Some((delim as char).to_string()),
    })
}

fn read_workbook(path: &Path, sheet: Option<&str>, limit: usize, file_size: u64) -> AppResult<TabularData> {
    let mut wb = open_workbook_auto(path).map_err(|e| AppError::Other(format!("hoja de cálculo: {e}")))?;
    let sheets = wb.sheet_names();
    let name = match sheet.filter(|s| !s.is_empty()) {
        Some(s) if sheets.iter().any(|n| n == s) => s.to_string(),
        Some(s) => return Err(AppError::NotFound(format!("hoja {s}"))),
        None => sheets.first().cloned().ok_or_else(|| AppError::Other("el libro no tiene hojas".into()))?,
    };
    let range = wb.worksheet_range(&name).map_err(|e| AppError::Other(format!("hoja {name}: {e}")))?;
    let mut it = range.rows();
    let headers: Vec<String> = it.next().map(|r| r.iter().map(|c| c.to_string()).collect()).unwrap_or_default();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut total = 0usize;
    for r in it {
        total += 1;
        if rows.len() < limit {
            rows.push(r.iter().map(|c| c.to_string()).collect());
        }
    }
    Ok(TabularData { sheets, sheet: Some(name), headers, rows, total_rows: total, file_size, delimiter: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_comma_with_limit_total_quotes_and_bom() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.csv");
        let mut s = String::from("\u{feff}id,name\n");
        for i in 0..10 {
            s.push_str(&format!("{i},\"n,{i}\"\n"));
        }
        std::fs::write(&p, s).unwrap();
        let t = read(&p, None, 3).unwrap();
        assert_eq!(t.headers, vec!["id", "name"]);
        assert_eq!(t.rows.len(), 3);
        assert_eq!(t.total_rows, 10);
        assert_eq!(t.rows[1], vec!["1", "n,1"]);
        assert_eq!(t.delimiter.as_deref(), Some(","));
        assert!(t.sheets.is_empty() && t.sheet.is_none());
        assert!(t.file_size > 0);
    }

    #[test]
    fn delimiter_detection_semicolon_tab_and_tsv() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.csv");
        std::fs::write(&p, "a;b;c\n1;2;3\n").unwrap();
        let t = read(&p, None, 10).unwrap();
        assert_eq!((t.delimiter.as_deref(), t.headers.len()), (Some(";"), 3));
        let p = d.path().join("t.txt");
        std::fs::write(&p, "a\tb\n1\t2\n").unwrap();
        assert_eq!(read(&p, None, 10).unwrap().delimiter.as_deref(), Some("\t"));
        let p = d.path().join("u.tsv");
        std::fs::write(&p, "a,b\tc\n1,2\t3\n").unwrap();
        let t = read(&p, None, 10).unwrap();
        assert_eq!(t.headers, vec!["a,b", "c"]);
    }

    #[test]
    fn ragged_rows_and_invalid_utf8_do_not_fail() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("r.csv");
        std::fs::write(&p, b"a,b\n1\n2,3,4\n\xff,x\n").unwrap();
        let t = read(&p, None, 10).unwrap();
        assert_eq!(t.total_rows, 3);
        assert_eq!(t.rows[1].len(), 3);
        assert!(t.rows[2][0].contains('\u{FFFD}'));
    }

    #[test]
    fn unsupported_and_missing() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("x.bin");
        std::fs::write(&p, "x").unwrap();
        assert!(read(&p, None, 10).is_err());
        assert!(read(&d.path().join("no.csv"), None, 10).is_err());
    }

    #[test]
    fn xlsx_roundtrip_via_minimal_workbook() {
        // xlsx mínimo hecho a mano (zip) para ejercitar calamine sin fixtures binarios.
        use std::io::Write;
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("w.xlsx");
        let f = File::create(&p).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default();
        let files: [(&str, &str); 5] = [
            ("[Content_Types].xml", r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#),
            ("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#),
            ("xl/workbook.xml", r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Datos" sheetId="1" r:id="rId1"/></sheets></workbook>"#),
            ("xl/_rels/workbook.xml.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#),
            ("xl/worksheets/sheet1.xml", r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>id</t></is></c><c r="B1" t="inlineStr"><is><t>nombre</t></is></c></row><row r="2"><c r="A2"><v>1</v></c><c r="B2" t="inlineStr"><is><t>uno</t></is></c></row><row r="3"><c r="A3"><v>2</v></c><c r="B3" t="inlineStr"><is><t>dos</t></is></c></row></sheetData></worksheet>"#),
        ];
        for (n, c) in files {
            z.start_file(n, o).unwrap();
            z.write_all(c.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        let t = read(&p, None, 1).unwrap();
        assert_eq!(t.sheets, vec!["Datos"]);
        assert_eq!(t.sheet.as_deref(), Some("Datos"));
        assert_eq!(t.headers, vec!["id", "nombre"]);
        assert_eq!(t.total_rows, 2);
        assert_eq!(t.rows, vec![vec!["1", "uno"]]);
        assert!(read(&p, Some("Nope"), 10).is_err());
    }
}
