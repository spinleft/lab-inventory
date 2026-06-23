use crate::domain::{UnitCode, UnitDimension, UnitName, UnitSymbol};

#[derive(Debug)]
pub struct UpdateUnit {
    pub code: Option<UnitCode>,
    pub name: Option<UnitName>,
    pub symbol: Option<UnitSymbol>,
    pub dimension: Option<UnitDimension>,
    pub scale_to_base: Option<f64>,
    pub allow_decimal: Option<bool>,
}

impl UpdateUnit {
    pub fn new(
        code: Option<UnitCode>,
        name: Option<UnitName>,
        symbol: Option<UnitSymbol>,
        dimension: Option<UnitDimension>,
        scale_to_base: Option<f64>,
        allow_decimal: Option<bool>,
    ) -> Result<Self, String> {
        if let Some(scale_to_base) = scale_to_base {
            crate::domain::validate_scale_to_base(scale_to_base)?;
        }
        Ok(Self {
            code,
            name,
            symbol,
            dimension,
            scale_to_base,
            allow_decimal,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::UpdateUnit;
    use crate::domain::{UnitCode, UnitDimension, UnitName, UnitSymbol};
    use claims::assert_err;

    #[test]
    fn update_unit_captures_partial_updates() {
        let update = UpdateUnit::new(
            Some(UnitCode::parse("inch".into()).unwrap()),
            Some(UnitName::parse("Inch".into()).unwrap()),
            Some(UnitSymbol::parse("in".into()).unwrap()),
            Some(UnitDimension::parse("length").unwrap()),
            Some(0.0254),
            Some(true),
        )
        .unwrap();

        assert_eq!(update.code.as_ref().map(|code| code.as_ref()), Some("inch"));
        assert_eq!(update.name.as_ref().map(|name| name.as_ref()), Some("Inch"));
        assert_eq!(
            update.symbol.as_ref().map(|symbol| symbol.as_ref()),
            Some("in")
        );
        assert_eq!(
            update
                .dimension
                .as_ref()
                .map(|dimension| dimension.as_ref()),
            Some("length")
        );
        assert_eq!(update.scale_to_base, Some(0.0254));
        assert_eq!(update.allow_decimal, Some(true));
    }

    #[test]
    fn update_unit_rejects_invalid_scale_to_base() {
        assert_err!(UpdateUnit::new(None, None, None, None, Some(0.0), None));
    }
}
