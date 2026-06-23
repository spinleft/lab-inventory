use crate::domain::{UnitCode, UnitDimension, UnitName, UnitSymbol};

#[derive(Debug)]
pub struct NewUnit {
    pub code: UnitCode,
    pub name: UnitName,
    pub symbol: UnitSymbol,
    pub dimension: UnitDimension,
    pub scale_to_base: f64,
    pub allow_decimal: bool,
}

impl NewUnit {
    pub fn new(
        code: UnitCode,
        name: UnitName,
        symbol: UnitSymbol,
        dimension: UnitDimension,
        scale_to_base: f64,
        allow_decimal: bool,
    ) -> Result<Self, String> {
        validate_scale_to_base(scale_to_base)?;
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

pub fn validate_scale_to_base(scale_to_base: f64) -> Result<(), String> {
    if scale_to_base.is_finite() && scale_to_base > 0.0 {
        Ok(())
    } else {
        Err("scale_to_base must be greater than zero".into())
    }
}

#[cfg(test)]
mod tests {
    use super::NewUnit;
    use crate::domain::{UnitCode, UnitDimension, UnitName, UnitSymbol};
    use claims::{assert_err, assert_ok};

    #[test]
    fn new_unit_accepts_positive_scale_to_base() {
        assert_ok!(NewUnit::new(
            UnitCode::parse("inch".into()).unwrap(),
            UnitName::parse("Inch".into()).unwrap(),
            UnitSymbol::parse("in".into()).unwrap(),
            UnitDimension::parse("length").unwrap(),
            0.0254,
            true,
        ));
    }

    #[test]
    fn new_unit_rejects_invalid_scale_to_base() {
        for scale_to_base in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            assert_err!(NewUnit::new(
                UnitCode::parse("bad".into()).unwrap(),
                UnitName::parse("Bad".into()).unwrap(),
                UnitSymbol::parse("bad".into()).unwrap(),
                UnitDimension::parse("length").unwrap(),
                scale_to_base,
                true,
            ));
        }
    }
}
