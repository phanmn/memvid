use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;
use zip::ZipArchive;

use crate::{
    DocumentFormat, DocumentReader, PassthroughReader, ReaderDiagnostics, ReaderHint, ReaderOutput,
    Result,
};

const SLIDE_PREFIX: &str = "ppt/slides/slide";
const SLIDE_SUFFIX: &str = ".xml";

pub struct PptxReader;

impl PptxReader {
    fn extract_text(bytes: &[u8]) -> Result<String> {
        let cursor = Cursor::new(bytes);
        let mut archive =
            ZipArchive::new(cursor).map_err(|err| crate::MemvidError::ExtractionFailed {
                reason: format!("failed to open pptx archive: {err}").into(),
            })?;

        let mut slides: Vec<String> = Vec::new();
        for i in 1..=archive.len() {
            let name = format!("{SLIDE_PREFIX}{i}{SLIDE_SUFFIX}");
            if let Ok(mut file) = archive.by_name(&name) {
                let mut xml = String::new();
                file.read_to_string(&mut xml).map_err(|err| {
                    crate::MemvidError::ExtractionFailed {
                        reason: format!("failed to read {name}: {err}").into(),
                    }
                })?;
                slides.push(xml);
            }
        }

        if slides.is_empty() {
            return Ok(String::new());
        }

        let mut out = String::new();
        for (idx, xml) in slides.iter().enumerate() {
            if idx > 0 {
                out.push_str("\n\n");
            }
            out.push_str(&format!("Slide {}:\n", idx + 1));
            out.push_str(&extract_plain_text(xml, b"p"));
        }

        Ok(out.trim().to_string())
    }
}

impl DocumentReader for PptxReader {
    fn name(&self) -> &'static str {
        "pptx"
    }

    fn supports(&self, hint: &ReaderHint<'_>) -> bool {
        matches!(hint.format, Some(DocumentFormat::Pptx))
            || hint.mime.is_some_and(|mime| {
                mime.eq_ignore_ascii_case(
                    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
                )
            })
    }

    fn extract(&self, bytes: &[u8], hint: &ReaderHint<'_>) -> Result<ReaderOutput> {
        match Self::extract_text(bytes) {
            Ok(text) => {
                if text.trim().is_empty() {
                    // quick-xml returned empty - try extractous as fallback
                    let mut fallback = PassthroughReader.extract(bytes, hint)?;
                    fallback.reader_name = self.name().to_string();
                    fallback.diagnostics.mark_fallback();
                    fallback.diagnostics.record_warning(
                        "pptx reader produced empty text; falling back to default extractor",
                    );
                    Ok(fallback)
                } else {
                    // quick-xml succeeded - build output directly WITHOUT calling extractous
                    let mut document = crate::ExtractedDocument::empty();
                    document.text = Some(text);
                    document.mime_type = Some(
                        "application/vnd.openxmlformats-officedocument.presentationml.presentation"
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
                    .record_warning(format!("pptx reader error: {err}"));
                Ok(fallback)
            }
        }
    }
}

fn extract_plain_text(xml: &str, block_suffix: &[u8]) -> String {
    let mut reader = XmlReader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut text = String::new();
    let mut first_block = true;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref().ends_with(block_suffix) {
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
    fn pptx_reader_name() {
        let reader = PptxReader;
        assert_eq!(reader.name(), "pptx");
    }

    #[test]
    fn supports_correct_mime_type() {
        let reader = PptxReader;
        let hint = ReaderHint::new(
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
            None,
        );
        assert!(reader.supports(&hint));
    }

    #[test]
    fn supports_document_format_pptx() {
        let reader = PptxReader;
        let hint = ReaderHint::new(None, Some(DocumentFormat::Pptx));
        assert!(reader.supports(&hint));
    }

    #[test]
    fn rejects_pdf_mime_type() {
        let reader = PptxReader;
        let hint = ReaderHint::new(Some("application/pdf"), None);
        assert!(!reader.supports(&hint));
    }

    #[test]
    fn rejects_docx_mime_type() {
        let reader = PptxReader;
        let hint = ReaderHint::new(
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            None,
        );
        assert!(!reader.supports(&hint));
    }

    #[test]
    fn rejects_plain_text_format() {
        let reader = PptxReader;
        let hint = ReaderHint::new(None, Some(DocumentFormat::PlainText));
        assert!(!reader.supports(&hint));
    }

    #[test]
    fn invalid_bytes_returns_error_or_fallback() {
        let reader = PptxReader;
        let hint = ReaderHint::new(None, Some(DocumentFormat::Pptx));
        // Not a valid zip/pptx - extract_text will fail, then fallback to PassthroughReader
        // which may also fail on garbage bytes. Either way we should not panic.
        let result = reader.extract(b"this is not a pptx file", &hint);
        match result {
            Ok(output) => {
                assert_eq!(output.reader_name, "pptx");
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
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:txBody>
          <a:p>
            <a:r><a:t>Slide Title</a:t></a:r>
          </a:p>
          <a:p>
            <a:r><a:t>Bullet point one</a:t></a:r>
          </a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#;
        let result = extract_plain_text(xml, b"p");
        assert!(
            result.contains("Slide Title"),
            "should contain 'Slide Title', got: {result}"
        );
        assert!(
            result.contains("Bullet point one"),
            "should contain 'Bullet point one', got: {result}"
        );
    }

    #[test]
    fn extract_plain_text_empty_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree></p:spTree>
  </p:cSld>
</p:sld>"#;
        let result = extract_plain_text(xml, b"p");
        assert!(
            result.is_empty(),
            "empty slide should produce empty text, got: {result}"
        );
    }

    #[test]
    fn extract_plain_text_multiple_text_runs() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:txBody>
          <a:p>
            <a:r><a:t>First run</a:t></a:r>
            <a:r><a:t>Second run</a:t></a:r>
          </a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#;
        let result = extract_plain_text(xml, b"p");
        assert!(
            result.contains("First run"),
            "should contain 'First run', got: {result}"
        );
        assert!(
            result.contains("Second run"),
            "should contain 'Second run', got: {result}"
        );
    }
}
