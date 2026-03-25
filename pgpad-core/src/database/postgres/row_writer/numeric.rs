use bytes::Buf;
use std::error::Error;
use tokio_postgres::types::{FromSql, Type};

#[derive(Debug)]
pub struct PostgresNumeric {
    pub ndigits: u16,
    pub weight: i16,
    pub sign: u16,
    pub dscale: u16,
    pub digits: Vec<u16>,
}

fn write_digit_group_padded(f: &mut fmt::Formatter<'_>, digit: u16) -> fmt::Result {
    let d1 = digit / 1000;
    let d2 = (digit / 100) % 10;
    let d3 = (digit / 10) % 10;
    let d4 = digit % 10;

    write!(f, "{}{}{}{}", d1, d2, d3, d4)
}

// Write individual digits from a digit group, up to a limit, returning digits written
fn write_digit_group_limited(
    f: &mut fmt::Formatter<'_>,
    digit: u16,
    limit: usize,
) -> Result<usize, fmt::Error> {
    let digits = [
        digit / 1000,
        (digit / 100) % 10,
        (digit / 10) % 10,
        digit % 10,
    ];

    let mut written = 0;
    for &d in &digits {
        if written >= limit {
            break;
        }
        write!(f, "{}", d)?;
        written += 1;
    }

    Ok(written)
}

impl<'a> FromSql<'a> for PostgresNumeric {
    fn from_sql(_: &Type, mut raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let ndigits = raw.get_u16();
        let weight = raw.get_i16();
        let sign = raw.get_u16();
        let dscale = raw.get_u16();

        let mut digits = Vec::with_capacity(ndigits as usize);
        for _ in 0..ndigits {
            digits.push(raw.get_u16());
        }

        Ok(PostgresNumeric {
            ndigits,
            weight,
            sign,
            dscale,
            digits,
        })
    }

    fn accepts(ty: &Type) -> bool {
        matches!(*ty, Type::NUMERIC)
    }
}

use std::fmt;

impl fmt::Display for PostgresNumeric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.sign == 0xC000 {
            return write!(f, "NaN");
        }

        if self.ndigits == 0 {
            if self.dscale > 0 {
                write!(f, "0.")?;
                for _ in 0..self.dscale {
                    write!(f, "0")?;
                }
            } else {
                write!(f, "0")?;
            }
            return Ok(());
        }

        // Check if the number is negative
        if self.sign == 0x4000 {
            write!(f, "-")?;
        }

        let weight = self.weight as i32;
        let num_groups = self.digits.len() as i32;
        let groups_before_decimal = weight + 1;
        let groups_after_decimal = num_groups - groups_before_decimal;

        // Integer part
        if groups_before_decimal <= 0 {
            write!(f, "0")?;
        } else {
            let groups_before = groups_before_decimal as usize;
            for (i, &digit) in self.digits.iter().take(groups_before).enumerate() {
                if i == 0 {
                    write!(f, "{}", digit)?;
                } else {
                    write_digit_group_padded(f, digit)?;
                }
            }

            if groups_after_decimal < 0 {
                for _ in 0..(-groups_after_decimal) {
                    write!(f, "0000")?;
                }
            }
        }

        // Write the decimal part, if there's one
        if self.dscale > 0 {
            write!(f, ".")?;

            let mut fractional_digits_written = 0;
            let dscale = self.dscale as usize;

            // leading zeros for fractional numbers
            if groups_before_decimal <= 0 {
                let leading_zero_groups = -groups_before_decimal;
                for _ in 0..leading_zero_groups {
                    let zeros_to_write = (4).min(dscale - fractional_digits_written);
                    for _ in 0..zeros_to_write {
                        write!(f, "0")?;
                        fractional_digits_written += 1;
                        if fractional_digits_written >= dscale {
                            return Ok(());
                        }
                    }
                }

                // Write fractional digit groups
                for &digit in self.digits.iter() {
                    if fractional_digits_written >= dscale {
                        break;
                    }
                    let remaining = dscale - fractional_digits_written;
                    let written = write_digit_group_limited(f, digit, remaining)?;
                    fractional_digits_written += written;
                }
            } else if groups_after_decimal > 0 {
                // Write fractional groups for mixed numbers
                let groups_before = groups_before_decimal as usize;
                for &digit in self.digits.iter().skip(groups_before) {
                    if fractional_digits_written >= dscale {
                        break;
                    }
                    let remaining = dscale - fractional_digits_written;
                    let written = write_digit_group_limited(f, digit, remaining)?;
                    fractional_digits_written += written;
                }
            }

            // trailing zeros if needed
            while fractional_digits_written < dscale {
                write!(f, "0")?;
                fractional_digits_written += 1;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_postgres::types::{FromSql, Type};

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn numeric_display() {
        let test_cases = vec![
            // Obtained with SELECT encode(numeric_send('12345.6789'::numeric), 'hex');
            ("0003000100000004000109291a85", "12345.6789"),
            // Obtained with SELECT encode(numeric_send('0.12345'::numeric), 'hex');
            ("0002ffff0000000504d21388", "0.12345"),
            // Obtained with SELECT encode(numeric_send('-123.45'::numeric), 'hex');
            ("0002000040000002007b1194", "-123.45"),
            // Obtained with SELECT encode(numeric_send('0'::numeric), 'hex');
            ("0000000000000000", "0"),
            // Obtained with SELECT encode(numeric_send('123456789'::numeric), 'hex');
            ("0003000200000000000109291a85", "123456789"),
            // Obtained with SELECT encode(numeric_send('0.000001'::numeric), 'hex');
            ("0001fffe000000060064", "0.000001"),
            // Obtained with SELECT encode(numeric_send('123.4'::numeric(10,4)), 'hex');
            ("0002000000000004007b0fa0", "123.4000"),
            // Obtained with SELECT encode(numeric_send('100'::numeric(8,2)), 'hex');
            ("00010000000000020064", "100.00"),
            // Obtained with SELECT encode(numeric_send('1.23e-10'::numeric), 'hex');
            ("0001fffd0000000c007b", "0.000000000123"),
        ];

        for (hex, expected) in test_cases {
            let bytes = hex_to_bytes(hex);
            let numeric = PostgresNumeric::from_sql(&Type::NUMERIC, &bytes).unwrap();
            let result = format!("{}", numeric);

            assert_eq!(
                result, expected,
                "Failed for hex {}: expected '{}', got '{}'",
                hex, expected, result
            );
        }
    }

    #[test]
    fn test_parse_structure() {
        // Obtained with SELECT encode(numeric_send('-123.45'::numeric), 'hex');
        let hex = "0002000040000002007b1194";
        let bytes = hex_to_bytes(hex);
        let numeric = PostgresNumeric::from_sql(&Type::NUMERIC, &bytes).unwrap();

        assert_eq!(numeric.ndigits, 2);
        assert_eq!(numeric.weight, 0);
        // Negative sign
        assert_eq!(numeric.sign, 0x4000);
        assert_eq!(numeric.dscale, 2);
        // 0x007b=123, 0x1194=4500
        assert_eq!(numeric.digits, vec![123, 4500]);

        // Obtained with SELECT encode(numeric_send('0'::numeric), 'hex');
        let hex = "0000000000000000";
        let bytes = hex_to_bytes(hex);
        let numeric = PostgresNumeric::from_sql(&Type::NUMERIC, &bytes).unwrap();

        assert_eq!(numeric.ndigits, 0);
        assert_eq!(numeric.weight, 0);
        assert_eq!(numeric.sign, 0x0000);
        assert_eq!(numeric.dscale, 0);
        assert_eq!(numeric.digits.len(), 0);

        // Obtained with SELECT encode(numeric_send('123.4'::numeric(10,4)), 'hex');
        let hex = "0002000000000004007b0fa0";
        let bytes = hex_to_bytes(hex);
        let numeric = PostgresNumeric::from_sql(&Type::NUMERIC, &bytes).unwrap();

        assert_eq!(numeric.ndigits, 2);
        assert_eq!(numeric.weight, 0);
        assert_eq!(numeric.sign, 0x0000);
        // NUMERIC(10,4) forces 4 decimal places
        assert_eq!(numeric.dscale, 4);
        // 0x007b=123, 0x0fa0=4000
        assert_eq!(numeric.digits, [123, 4000]);
    }
}
