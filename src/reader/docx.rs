use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;
use zip::ZipArchive;

use crate::{
    DocumentFormat, DocumentReader, PassthroughReader, ReaderDiagnostics, ReaderHint, ReaderOutput,
    Result,
};

const DOC_XML_PATH: &str = "word/document.xml";

pub struct DocxReader;

impl DocxReader {
    fn extract_text(bytes: &[u8]) -> Result<String> {
        let cursor = Cursor::new(bytes);
        let mut archive =
            ZipArchive::new(cursor).map_err(|err| crate::MemvidError::ExtractionFailed {
                reason: format!("failed to open docx archive: {err}").into(),
            })?;

        let mut file =
            archive
                .by_name(DOC_XML_PATH)
                .map_err(|err| crate::MemvidError::ExtractionFailed {
                    reason: format!("docx missing document.xml: {err}").into(),
                })?;
        let mut xml = String::new();
        file.read_to_string(&mut xml)
            .map_err(|err| crate::MemvidError::ExtractionFailed {
                reason: format!("failed to read document.xml: {err}").into(),
            })?;

        Ok(extract_plain_text(&xml, b"w:p"))
    }
}

impl DocumentReader for DocxReader {
    fn name(&self) -> &'static str {
        "docx"
    }

    fn supports(&self, hint: &ReaderHint<'_>) -> bool {
        matches!(hint.format, Some(DocumentFormat::Docx))
            || hint.mime.is_some_and(|mime| {
                mime.eq_ignore_ascii_case(
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                )
            })
    }

    fn extract(&self, bytes: &[u8], hint: &ReaderHint<'_>) -> Result<ReaderOutput> {
        match Self::extract_text(bytes) {
            Ok(text) => {
                if text.trim().is_empty() {
                    // quick-xml returned empty - try extractous as fallback
                    let mut output = PassthroughReader.extract(bytes, hint)?;
                    output.reader_name = self.name().to_string();
                    output.diagnostics.mark_fallback();
                    output.diagnostics.record_warning(
                        "docx reader produced empty text; falling back to default extractor",
                    );
                    Ok(output)
                } else {
                    // quick-xml succeeded - build output directly WITHOUT calling extractous
                    let mut document = crate::ExtractedDocument::empty();
                    document.text = Some(text);
                    document.mime_type = Some(
                        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                            .to_string(),
                    );
                    Ok(ReaderOutput::new(document, self.name())
                        .with_diagnostics(ReaderDiagnostics::default()))
                }
            }
            Err(err) => {
                // quick-xml failed - try extractous as fallback
                let mut fallback = PassthroughReader.extract(bytes, hint)?;
                fallback.reader_name = self.name().to_string();
                fallback.diagnostics.mark_fallback();
                fallback
                    .diagnostics
                    .record_warning(format!("docx reader error: {err}"));
                Ok(fallback)
            }
        }
    }
}

fn extract_plain_text(xml: &str, block_tag: &[u8]) -> String {
    let mut reader = XmlReader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut text = String::new();
    let mut first_block = true;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref().ends_with(block_tag) {
                    if !first_block {
                        text.push('\n');
                    }
                    first_block = false;
                }
            }
            Ok(Event::Text(t)) => {
                if let Ok(content) = t.unescape() {
                    if !content.trim().is_empty() {
                        text.push_str(content.trim());
                        text.push(' ');
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => (),
        }
        buf.clear();
    }

    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docx_reader_name() {
        let reader = DocxReader;
        assert_eq!(reader.name(), "docx");
    }

    #[test]
    fn supports_correct_mime_type() {
        let reader = DocxReader;
        let hint = ReaderHint::new(
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            None,
        );
        assert!(reader.supports(&hint));
    }

    #[test]
    fn supports_document_format_docx() {
        let reader = DocxReader;
        let hint = ReaderHint::new(None, Some(DocumentFormat::Docx));
        assert!(reader.supports(&hint));
    }

    #[test]
    fn rejects_pdf_mime_type() {
        let reader = DocxReader;
        let hint = ReaderHint::new(Some("application/pdf"), None);
        assert!(!reader.supports(&hint));
    }

    #[test]
    fn rejects_pptx_mime_type() {
        let reader = DocxReader;
        let hint = ReaderHint::new(
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
            None,
        );
        assert!(!reader.supports(&hint));
    }

    #[test]
    fn rejects_plain_text_format() {
        let reader = DocxReader;
        let hint = ReaderHint::new(None, Some(DocumentFormat::PlainText));
        assert!(!reader.supports(&hint));
    }

    #[test]
    fn invalid_bytes_returns_error_or_fallback() {
        let reader = DocxReader;
        let hint = ReaderHint::new(None, Some(DocumentFormat::Docx));
        // Not a valid zip/docx - extract_text will fail, then fallback to PassthroughReader
        // which may also fail on garbage bytes. Either way we should not panic.
        let result = reader.extract(b"this is not a docx file", &hint);
        // The extract method falls back to PassthroughReader on error, so it may
        // succeed with empty/garbage text or fail. The key assertion is no panic.
        match result {
            Ok(output) => {
                assert_eq!(output.reader_name, "docx");
                assert!(output.diagnostics.fallback);
            }
            Err(_) => {
                // Also acceptable - extraction failed for invalid data
            }
        }
    }

    #[test]
    fn extract_plain_text_from_sample_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>Hello World</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>Second paragraph</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let result = extract_plain_text(xml, b"w:p");
        assert!(result.contains("Hello World"), "should contain 'Hello World', got: {result}");
        assert!(
            result.contains("Second paragraph"),
            "should contain 'Second paragraph', got: {result}"
        );
    }

    #[test]
    fn extract_plain_text_empty_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body></w:body>
</w:document>"#;
        let result = extract_plain_text(xml, b"w:p");
        assert!(result.is_empty(), "empty document should produce empty text, got: {result}");
    }

    #[test]
    fn extract_plain_text_with_whitespace_only() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>   </w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let result = extract_plain_text(xml, b"w:p");
        assert!(
            result.is_empty(),
            "whitespace-only content should be trimmed to empty, got: {result}"
        );
    }
}
