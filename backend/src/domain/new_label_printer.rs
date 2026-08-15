use crate::domain::{LabelPrinterHost, LabelPrinterMedia, LabelPrinterName};
use crate::label_printing::MIN_PRINTER_PORT;

#[derive(Debug)]
pub struct NewLabelPrinter {
    pub name: LabelPrinterName,
    pub host: LabelPrinterHost,
    pub port: u16,
    pub model: String,
    pub media: LabelPrinterMedia,
    pub auto_cut: bool,
}

impl NewLabelPrinter {
    pub fn new(
        name: LabelPrinterName,
        host: LabelPrinterHost,
        port: i32,
        model: String,
        media: LabelPrinterMedia,
        auto_cut: bool,
    ) -> Result<Self, String> {
        Ok(Self {
            name,
            host,
            port: validate_port(port)?,
            model: validate_model(model)?,
            media,
            auto_cut,
        })
    }
}

#[derive(Debug, Default)]
pub struct UpdateLabelPrinter {
    pub name: Option<LabelPrinterName>,
    pub host: Option<LabelPrinterHost>,
    pub port: Option<u16>,
    pub model: Option<String>,
    pub media: Option<LabelPrinterMedia>,
    pub auto_cut: Option<bool>,
}

impl UpdateLabelPrinter {
    /// Whether the request would change anything at all.
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.host.is_none()
            && self.port.is_none()
            && self.model.is_none()
            && self.media.is_none()
            && self.auto_cut.is_none()
    }
}

/// Printers listen on a registered port, never a privileged one.
///
/// The same floor is enforced again before connecting, so a row written by an
/// older version cannot be used to reach a privileged port either.
pub fn validate_port(port: i32) -> Result<u16, String> {
    let port = u16::try_from(port).map_err(|_| format!("{port} is not a valid port."))?;
    if port < MIN_PRINTER_PORT {
        return Err(format!(
            "Port {port} is not allowed; use the printer's raw printing port (usually 9100)."
        ));
    }
    Ok(port)
}

pub fn validate_model(model: String) -> Result<String, String> {
    let trimmed = model.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 64 {
        return Err(format!("{model} is not a valid printer model."));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::{UpdateLabelPrinter, validate_model, validate_port};
    use claims::{assert_err, assert_ok};

    #[test]
    fn registered_ports_are_accepted() {
        assert_eq!(validate_port(9100), Ok(9100));
        assert_eq!(validate_port(1024), Ok(1024));
        assert_eq!(validate_port(65535), Ok(65535));
    }

    #[test]
    fn privileged_and_out_of_range_ports_are_rejected() {
        assert_err!(validate_port(22));
        assert_err!(validate_port(1023));
        assert_err!(validate_port(0));
        assert_err!(validate_port(-1));
        assert_err!(validate_port(65536));
    }

    #[test]
    fn models_are_trimmed_and_bounded() {
        assert_eq!(
            validate_model("  QL-820NWBc  ".into()),
            Ok("QL-820NWBc".into())
        );
        assert_err!(validate_model(String::new()));
        assert_err!(validate_model("   ".into()));
        assert_ok!(validate_model("a".repeat(64)));
        assert_err!(validate_model("a".repeat(65)));
    }

    #[test]
    fn an_update_with_no_fields_set_is_empty() {
        assert!(UpdateLabelPrinter::default().is_empty());
    }

    #[test]
    fn an_update_touching_one_field_is_not_empty() {
        let update = UpdateLabelPrinter {
            auto_cut: Some(false),
            ..Default::default()
        };
        assert!(!update.is_empty());
    }
}
