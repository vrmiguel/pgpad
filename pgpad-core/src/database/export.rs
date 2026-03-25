use anyhow::{bail, ensure};
use jsax::Event;
use std::fmt::Write;

fn write_nested_item(
    parser: &mut jsax::Parser<'_>,
    opening: Event<'_>,
    out: &mut String,
) -> anyhow::Result<()> {
    match opening {
        Event::StartObject => out.push('{'),
        Event::StartArray => out.push('['),
        _ => bail!("write_nested_item called with non-container event"),
    }

    let mut nesting: usize = 1;
    // Whether the next cell is the first in its row
    let mut first = true;

    while nesting > 0 {
        let event = parser
            .parse_next()?
            .ok_or_else(|| anyhow::anyhow!("Unexpected end of input inside nested value"))?;

        match event {
            Event::StartObject => {
                write_sep(out, &mut first);
                out.push('{');
                nesting += 1;
                first = true;
            }
            Event::EndObject { .. } => {
                out.push('}');
                nesting -= 1;
                first = false;
            }
            Event::StartArray => {
                write_sep(out, &mut first);
                out.push('[');
                nesting += 1;
                first = true;
            }
            Event::EndArray { .. } => {
                out.push(']');
                nesting -= 1;
                first = false;
            }
            Event::Key(key) => {
                write_sep(out, &mut first);
                write!(out, "\"{key}\":")?;
                // can't add a comma next, since we now need to write the element that follows the `"key":`.
                // we regardless set up the flag since the element _after_ it will be preceded by a comma
                first = true;
            }
            Event::String(s) => {
                write_sep(out, &mut first);
                write!(out, "\"{s}\"")?;
            }
            Event::Number(n) => {
                write_sep(out, &mut first);
                write!(out, "{n}")?;
            }
            Event::Boolean(b) => {
                write_sep(out, &mut first);
                out.push_str(if b { "true" } else { "false" });
            }
            Event::Null => {
                write_sep(out, &mut first);
                out.push_str("null");
            }
        }
    }

    Ok(())
}

fn write_cell_value(
    parser: &mut jsax::Parser<'_>,
    event: Event<'_>,
    out: &mut String,
) -> anyhow::Result<()> {
    match event {
        Event::String(s) => write!(out, "\"{s}\"")?,
        Event::Number(n) => write!(out, "{n}")?,
        Event::Boolean(b) => out.push_str(if b { "true" } else { "false" }),
        Event::Null => out.push_str("null"),
        Event::StartArray | Event::StartObject => write_nested_item(parser, event, out)?,
        _ => bail!("Unexpected event for cell value"),
    }
    Ok(())
}

/// Writes a comma if this isn't the first element of the row then marks it as no longer first
fn write_sep(out: &mut String, first: &mut bool) {
    if !*first {
        out.push(',');
    }
    *first = false;
}

fn write_columns(out: &mut String, columns: &str) -> anyhow::Result<()> {
    let mut parser = jsax::Parser::new(columns);
    ensure!(
        matches!(parser.parse_next()?, Some(Event::StartArray)),
        r#"Columns should be in the format `["col1", "col2", "col3"]"#
    );

    let mut first = true;
    while let Some(event) = parser.parse_next()? {
        match event {
            Event::EndArray { len: _ } => {}
            Event::String(column_name) => {
                if first {
                    first = false;
                } else {
                    write!(out, ",")?;
                }
                write!(out, "{column_name}")?;
            }
            other => {
                bail!("In write_columns: Expected either EndArray (']'), or strings, but found '{:?}'", other);
            }
        }
    }

    writeln!(out)?;

    Ok(())
}

pub fn export_to_csv(columns: &str, page: &str) -> anyhow::Result<String> {
    ensure!(!columns.is_empty(), "Expected at least one column");

    let mut output = String::with_capacity(page.len());
    let mut parser = jsax::Parser::new(page);

    write_columns(&mut output, columns)?;

    let first_event = parser.parse_next()?;
    ensure!(
        matches!(first_event, Some(Event::StartArray)),
        "Expected page to start with an array"
    );

    let mut depth: usize = 1;
    // Whether the next cell is the first in its row
    let mut first_cell = true;

    while let Some(event) = parser.parse_next()? {
        match event {
            Event::StartArray if depth == 1 => {
                depth = 2;
                first_cell = true;
            }
            Event::EndArray { .. } if depth == 2 => {
                depth = 1;
                output.push('\n');
            }
            Event::EndArray { .. } if depth == 1 => break,
            cell_event => {
                write_sep(&mut output, &mut first_cell);
                write_cell_value(&mut parser, cell_event, &mut output)?;
            }
        }
    }

    Ok(output)
}

// Tests for CSVs exports are alongside tests for the StatementManager
