// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Markdown to Pango markup converter for GTK text display

use pulldown_cmark::{Parser, Event, Tag, TagEnd, HeadingLevel};

/// Convert markdown to Pango markup for GTK TextView
pub fn markdown_to_pango(markdown: &str) -> String {
    let parser = Parser::new(markdown);
    let mut output = String::new();
    let mut in_heading = false;
    let mut heading_level = 1;
    let mut in_code_block = false;
    let mut in_list_item = false;
    let mut list_depth: usize = 0;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    in_heading = true;
                    heading_level = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };

                    // Use larger, bold text for headings
                    let size = match heading_level {
                        1 => "xx-large",
                        2 => "x-large",
                        3 => "large",
                        _ => "medium",
                    };
                    output.push_str(&format!("<span size='{}' weight='bold'>", size));
                }
                Tag::Paragraph => {
                    // Don't add extra spacing in list items
                    if !in_list_item {
                        output.push_str("\n");
                    }
                }
                Tag::CodeBlock(_) => {
                    in_code_block = true;
                    output.push_str("\n<span font_family='monospace' background='#f0f0f0'>");
                }
                Tag::List(_) => {
                    list_depth += 1;
                    output.push_str("\n");
                }
                Tag::Item => {
                    in_list_item = true;
                    // Add indentation for nested lists
                    let indent = "  ".repeat(list_depth.saturating_sub(1));
                    output.push_str(&format!("\n{}• ", indent));
                }
                Tag::Strong => {
                    output.push_str("<b>");
                }
                Tag::Emphasis => {
                    output.push_str("<i>");
                }
                Tag::Link { .. } => {
                    output.push_str("<span foreground='blue' underline='single'>");
                }
                Tag::BlockQuote(_) => {
                    output.push_str("\n<span style='italic' foreground='#666666'>");
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    output.push_str("</span>\n");
                    in_heading = false;
                }
                TagEnd::Paragraph => {
                    if !in_list_item {
                        output.push_str("\n");
                    }
                }
                TagEnd::CodeBlock => {
                    output.push_str("</span>\n");
                    in_code_block = false;
                }
                TagEnd::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                    output.push_str("\n");
                }
                TagEnd::Item => {
                    in_list_item = false;
                }
                TagEnd::Strong => {
                    output.push_str("</b>");
                }
                TagEnd::Emphasis => {
                    output.push_str("</i>");
                }
                TagEnd::Link => {
                    output.push_str("</span>");
                }
                TagEnd::BlockQuote(_) => {
                    output.push_str("</span>\n");
                }
                _ => {}
            },
            Event::Text(text) => {
                // Escape XML special characters for Pango
                let escaped = text
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");

                if in_code_block {
                    // Preserve whitespace in code blocks
                    output.push_str(&escaped);
                } else {
                    output.push_str(&escaped);
                }
            }
            Event::Code(code) => {
                // Inline code
                let escaped = code
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                output.push_str(&format!("<span font_family='monospace' background='#f0f0f0'>{}</span>", escaped));
            }
            Event::SoftBreak => {
                output.push(' ');
            }
            Event::HardBreak => {
                output.push('\n');
            }
            Event::Rule => {
                output.push_str("\n───────────────────────────────────\n");
            }
            _ => {}
        }
    }

    output
}
