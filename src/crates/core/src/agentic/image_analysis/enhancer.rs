//! Message Enhancer
//!
//! Synthesizes image analysis results and other context into user messages

use super::types::ImageAnalysisResult;
use serde_json::Value;

/// Message Enhancer
pub struct MessageEnhancer;

impl MessageEnhancer {
    /// Synthesize enhanced message
    ///
    /// Combines original user message, image analysis results, and other context into a complete message
    pub fn enhance_with_image_analysis(
        original_message: &str,
        image_analyses: &[ImageAnalysisResult],
        other_contexts: &[Value],
    ) -> String {
        let mut enhanced = String::new();

        // 1. Image analysis section
        if !image_analyses.is_empty() {
            enhanced.push_str("User uploaded ");
            enhanced.push_str(&image_analyses.len().to_string());
            enhanced
                .push_str(" image(s). AI's understanding of the image content is as follows:\n\n");

            for (idx, analysis) in image_analyses.iter().enumerate() {
                enhanced.push_str(&format!("[Image {}]\n", idx + 1));
                enhanced.push_str(&format!("• Summary: {}\n", analysis.summary));
                enhanced.push_str(&format!(
                    "• Detailed description: {}\n",
                    analysis.detailed_description
                ));

                if !analysis.detected_elements.is_empty() {
                    enhanced.push_str("• Key elements: ");
                    enhanced.push_str(&analysis.detected_elements.join(", "));
                    enhanced.push_str("\n");
                }

                enhanced.push_str(&format!(
                    "• Analysis confidence: {:.1}%\n",
                    analysis.confidence * 100.0
                ));

                enhanced.push_str("\n");
            }
        }

        // 2. Other contexts (files, code snippets, etc.)
        if !other_contexts.is_empty() {
            enhanced.push_str("User also provided the following context information:\n\n");
            for ctx in other_contexts {
                if let Some(formatted) = Self::format_context(ctx) {
                    enhanced.push_str(&formatted);
                    enhanced.push_str("\n");
                }
            }
            enhanced.push_str("\n");
        }

        // 3. Separator
        enhanced.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

        // 4. Original user message
        enhanced.push_str("User's question:\n");
        enhanced.push_str(original_message);

        enhanced
    }

    /// Format context
    fn format_context(ctx: &Value) -> Option<String> {
        let ctx_type = ctx.get("type")?.as_str()?;

        match ctx_type {
            "file" => {
                let path = ctx.get("path")?.as_str()?;
                Some(format!("• File: {}", path))
            }
            "code-snippet" => {
                let file_name = ctx.get("fileName")?.as_str()?;
                let start_line = ctx.get("startLine")?.as_u64()?;
                let end_line = ctx.get("endLine")?.as_u64()?;
                Some(format!(
                    "• Code snippet: {} (lines {}-{})",
                    file_name, start_line, end_line
                ))
            }
            "directory" => {
                let path = ctx.get("path")?.as_str()?;
                Some(format!("• Directory: {}", path))
            }
            "mermaid-diagram" => {
                let title = ctx
                    .get("diagramTitle")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Untitled");
                Some(format!("• Mermaid diagram: {}", title))
            }
            _ => Some(format!("• {}", ctx_type)),
        }
    }
}
