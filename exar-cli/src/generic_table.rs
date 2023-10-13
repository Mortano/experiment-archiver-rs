use std::io::Write;

use anyhow::Result;
use tabled::{builder::Builder, settings::Style};

pub struct GenericTable {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl GenericTable {
    pub fn new(header: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self { header, rows }
    }

    pub fn write_pretty<W: Write>(&self, mut writer: W) -> Result<()> {
        let mut table_builder = Builder::default();
        table_builder.set_header(&self.header);

        for row in &self.rows {
            table_builder.push_record(row);
        }

        let mut table = table_builder.build();

        // let (terminal_width, _) =
        //     termion::terminal_size().context("Can't determine terminal size")?;
        // table.with(Width::wrap(terminal_width as usize));
        // table.with(Modify::new(Rows::new(..)).with(Width::wrap(24)));
        table.with(Style::modern());

        write!(writer, "{table}")?;

        Ok(())
    }

    pub fn write_csv<W: Write>(&self, mut writer: W) -> Result<()> {
        let cleanup_str_for_csv = |s: &String| -> String {
            let must_be_quoted = s.contains(&['\n', '\r', ',']);
            if !must_be_quoted {
                s.to_owned()
            } else {
                format!("\"{s}\"")
            }
        };

        let header = self
            .header
            .iter()
            .map(cleanup_str_for_csv)
            .collect::<Vec<_>>()
            .join(",");
        writeln!(writer, "{header}")?;
        for (idx, row) in self.rows.iter().enumerate() {
            let row = row
                .iter()
                .map(cleanup_str_for_csv)
                .collect::<Vec<_>>()
                .join(",");
            if idx == self.rows.len() - 1 {
                write!(writer, "{row}")?;
            } else {
                writeln!(writer, "{row}")?
            }
        }
        Ok(())
    }
}
