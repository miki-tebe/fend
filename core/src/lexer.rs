use crate::err::{IntErr, Interrupt, NeverInterrupt};
use crate::interrupt::test_int;
use crate::num::{Base, Number};
use std::{
    convert::TryInto,
    fmt::{Display, Error, Formatter},
};

#[derive(Clone)]
pub enum Token {
    Num(Number),
    Ident(String),
    Symbol(Symbol),
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum Symbol {
    OpenParens,
    CloseParens,
    Add,
    Sub,
    Mul,
    Div,
    // In GNU mode only: | is division with higher precedence. Otherwise
    // it is an error
    InnerDiv,
    Pow,
    ArrowConversion,
    Factorial,
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        let s = match self {
            Self::OpenParens => "(",
            Self::CloseParens => ")",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::InnerDiv => "|",
            Self::Pow => "^",
            Self::ArrowConversion => "->",
            Self::Factorial => "!",
        };
        write!(f, "{}", s)?;
        Ok(())
    }
}

fn parse_char(input: &str) -> Result<(char, &str), IntErr<String, NeverInterrupt>> {
    if let Some(ch) = input.chars().next() {
        let (_, b) = input.split_at(ch.len_utf8());
        Ok((ch, b))
    } else {
        Err("Expected a character".to_string())?
    }
}

fn consume_char(input: &mut &str) -> Result<char, IntErr<String, NeverInterrupt>> {
    let (ch, remaining_input) = parse_char(input)?;
    *input = remaining_input;
    Ok(ch)
}

fn parse_ascii_digit(
    input: &str,
    base: Base,
) -> Result<(u8, &str), IntErr<String, NeverInterrupt>> {
    let (ch, input) = parse_char(input)?;
    let possible_digit = ch.to_digit(base.base_as_u8().into());
    if let Some(digit) = possible_digit.and_then(|d| <u32 as TryInto<u8>>::try_into(d).ok()) {
        Ok((digit, input))
    } else {
        Err(format!("Expected a digit, found '{}'", ch))?
    }
}

fn parse_fixed_char(input: &str, ch: char) -> Result<((), &str), IntErr<String, NeverInterrupt>> {
    let (parsed_ch, input) = parse_char(input)?;
    if parsed_ch == ch {
        Ok(((), input))
    } else {
        Err(format!("Expected '{}', found '{}'", parsed_ch, ch))?
    }
}

fn parse_digit_separator(input: &str) -> Result<((), &str), IntErr<String, NeverInterrupt>> {
    let (parsed_ch, input) = parse_char(input)?;
    if parsed_ch == '_' || parsed_ch == ',' {
        Ok(((), input))
    } else {
        Err(format!("Expected a digit separator, found {}", parsed_ch))?
    }
}

// Parses a plain integer with no whitespace and no base prefix.
// Leading minus sign is not allowed.
fn parse_integer<'a, I: Interrupt>(
    input: &'a str,
    allow_digit_separator: bool,
    allow_leading_zeroes: bool,
    base: Base,
    process_digit: &mut impl FnMut(u8) -> Result<(), IntErr<String, I>>,
) -> Result<((), &'a str), IntErr<String, I>> {
    let (digit, mut input) = parse_ascii_digit(input, base).map_err(IntErr::get_error)?;
    process_digit(digit)?;
    let leading_zero = digit == 0;
    let mut parsed_digit_separator;
    loop {
        if let Ok((_, remaining)) = parse_digit_separator(input) {
            input = remaining;
            parsed_digit_separator = true;
            if !allow_digit_separator {
                return Err("Digit separators are not allowed".to_string())?;
            }
        } else {
            parsed_digit_separator = false;
        }
        match parse_ascii_digit(input, base) {
            Err(_) => {
                if parsed_digit_separator {
                    return Err("Digit separators can only occur between digits".to_string())?;
                }
                break;
            }
            Ok((digit, next_input)) => {
                if leading_zero && !allow_leading_zeroes {
                    return Err("Integer literals cannot have leading zeroes".to_string())?;
                }
                process_digit(digit)?;
                input = next_input;
            }
        }
    }
    Ok(((), input))
}

fn parse_base_prefix(input: &str) -> Result<(Base, &str), IntErr<String, NeverInterrupt>> {
    // 0x -> 16
    // 0d -> 10
    // 0o -> 8
    // 0b -> 2
    // base# -> base (where 2 <= base <= 36)
    // case-sensitive, no whitespace allowed
    if let Ok((_, input)) = parse_fixed_char(input, '0') {
        let (ch, input) = parse_char(input)?;
        Ok((Base::from_zero_based_prefix_char(ch)?, input))
    } else {
        let mut custom_base: u8 = 0;
        let (_, input) = parse_integer(input, false, false, Base::default(), &mut |digit| {
            if custom_base > 3 {
                return Err("Base cannot be larger than 36".to_string())?;
            }
            custom_base = 10 * custom_base + digit;
            if custom_base > 36 {
                return Err("Base cannot be larger than 36".to_string())?;
            }
            Ok(())
        })?;
        if custom_base < 2 {
            return Err("Base must be at least 2".to_string())?;
        }
        let (_, input) = parse_fixed_char(input, '#')?;
        Ok((Base::from_custom_base(custom_base)?, input))
    }
}

fn parse_basic_number<'a, I: Interrupt>(
    input: &'a str,
    base: Base,
    allow_zero: bool,
    int: &I,
) -> Result<(Number, &'a str), IntErr<String, I>> {
    // parse integer component
    let mut res = Number::zero_with_base(base);
    let (_, mut input) = parse_integer(
        input,
        true,
        base.allow_leading_zeroes(),
        base,
        &mut |digit| {
            let base_as_u64: u64 = base.base_as_u8().into();
            res = res
                .clone()
                .mul(base_as_u64.into(), int)?
                .add(u64::from(digit).into(), int)?;
            Ok(())
        },
    )?;

    // parse decimal point and at least one digit
    if let Ok((_, remaining)) = parse_fixed_char(input, '.') {
        let (_, remaining) = parse_integer(remaining, true, true, base, &mut |digit| {
            res.add_digit_in_base(digit.into(), base, int)?;
            Ok(())
        })?;
        input = remaining;
    }

    if !allow_zero && res.is_zero() {
        return Err("Invalid number: 0".to_string())?;
    }

    // parse optional exponent, but only for base 10 and below
    if base.base_as_u8() <= 10 {
        if let Ok((_, remaining)) = parse_fixed_char(input, 'e') {
            // peek ahead to the next char to determine if we should continue parsing an exponent
            let abort = if let Ok((ch, _)) = parse_char(remaining) {
                // abort if there is a non-alphanumeric non-plus or minus char after 'e',
                // such as '(' or '/'
                !(ch.is_alphanumeric() || ch == '+' || ch == '-')
            } else {
                // if there is no more input after the 'e', abort
                true
            };
            if !abort {
                input = remaining;
                let mut negative_exponent = false;
                if let Ok((_, remaining)) = parse_fixed_char(input, '-') {
                    negative_exponent = true;
                    input = remaining;
                } else if let Ok((_, remaining)) = parse_fixed_char(input, '+') {
                    input = remaining;
                }
                let mut exp = Number::zero_with_base(base);
                let base_num = Number::from(u64::from(base.base_as_u8()));
                let (_, remaining) = parse_integer(input, true, true, base, &mut |digit| {
                    exp = (exp.clone().mul(base_num.clone(), int)?)
                        .add(u64::from(digit).into(), int)?;
                    Ok(())
                })?;
                if negative_exponent {
                    exp = -exp;
                }
                let base_as_u64: u64 = base.base_as_u8().into();
                let base_as_number: Number = base_as_u64.into();
                res = res.mul(base_as_number.pow(exp, int)?, int)?;
                input = remaining;
            }
        }
    }

    Ok((res, input))
}

fn parse_number<'a, I: Interrupt>(
    input: &'a str,
    int: &I,
) -> Result<(Number, &'a str), IntErr<String, I>> {
    let (base, input) = parse_base_prefix(input).unwrap_or((Base::default(), input));
    let (res, input) = parse_basic_number(input, base, true, int)?;
    Ok((res, input))
}

// checks if the char is valid only by itself
pub fn is_valid_in_ident_char(ch: char) -> bool {
    let allowed_symbols = "%‰‱′″\"'’”";
    allowed_symbols.contains(ch)
}

// normal rules for identifiers
pub fn is_valid_in_ident(ch: char, first: bool) -> bool {
    ch.is_alphabetic() || ",&_⅛¼⅜½⅝¾⅞⅙⅓⅔⅚⅕⅖⅗⅘°$℃℉℧℈℥℔¢£¥€₩₪₤₨฿₡₣₦₧₫₭₮₯₱﷼﹩￠￡￥￦㍱㍲㍳㍴㍶㎀㎁㎂㎃㎄㎅㎆㎇㎈㎉㎊㎋㎌㎍㎎㎏㎐㎑㎒㎓㎔㎕㎖㎗㎘㎙㎚㎛㎜㎝㎞㎟㎠㎡㎢㎣㎤㎥㎦㎧㎨㎩㎪㎫㎬㎭㎮㎯㎰㎱㎲㎳㎴㎵㎶㎷㎸㎹㎺㎻㎼㎽㎾㎿㏀㏁㏃㏄㏅㏆㏈㏉㏊㏌㏏㏐㏓㏔㏕㏖㏗㏙㏛㏜㏝".contains(ch) || (!first && ".0123456789".contains(ch))
}

fn parse_ident(input: &str) -> Result<(Token, &str), IntErr<String, NeverInterrupt>> {
    let (first_char, _) = parse_char(input)?;
    if !is_valid_in_ident(first_char, true) {
        if is_valid_in_ident_char(first_char) {
            let (first_char_str, input) = input.split_at(first_char.len_utf8());
            return Ok((Token::Ident(first_char_str.to_string()), input));
        }
        return Err(format!(
            "Character '{}' is not valid at the beginning of an identifier",
            first_char
        ))?;
    }
    let mut byte_idx = first_char.len_utf8();
    let (_, mut remaining) = input.split_at(byte_idx);
    while let Ok((next_char, remaining_input)) = parse_char(remaining) {
        if !is_valid_in_ident(next_char, false) {
            break;
        }
        remaining = remaining_input;
        byte_idx += next_char.len_utf8();
    }
    let (ident, input) = input.split_at(byte_idx);
    Ok((
        match ident {
            "to" | "as" => Token::Symbol(Symbol::ArrowConversion),
            "per" => Token::Symbol(Symbol::Div),
            _ => Token::Ident(ident.to_string()),
        },
        input,
    ))
}

pub fn lex<I: Interrupt>(mut input: &str, int: &I) -> Result<Vec<Token>, IntErr<String, I>> {
    let mut res = vec![];
    loop {
        test_int(int)?;
        match input.chars().next() {
            Some(ch) => {
                if ch.is_whitespace() {
                    consume_char(&mut input).map_err(IntErr::get_error)?;
                } else if ch.is_ascii_digit() {
                    let (num, remaining) = parse_number(input, int)?;
                    input = remaining;
                    res.push(Token::Num(num));
                } else if is_valid_in_ident(ch, true) || is_valid_in_ident_char(ch) {
                    let (ident, remaining) = parse_ident(input).map_err(IntErr::get_error)?;
                    input = remaining;
                    res.push(ident);
                } else {
                    match consume_char(&mut input).map_err(IntErr::get_error)? {
                        '(' => res.push(Token::Symbol(Symbol::OpenParens)),
                        ')' => res.push(Token::Symbol(Symbol::CloseParens)),
                        '+' => res.push(Token::Symbol(Symbol::Add)),
                        '!' => res.push(Token::Symbol(Symbol::Factorial)),
                        '-' => {
                            if input.starts_with('>') {
                                consume_char(&mut input).map_err(IntErr::get_error)?;
                                res.push(Token::Symbol(Symbol::ArrowConversion))
                            } else {
                                res.push(Token::Symbol(Symbol::Sub))
                            }
                        }
                        '*' => {
                            if input.starts_with('*') {
                                consume_char(&mut input).map_err(IntErr::get_error)?;
                                res.push(Token::Symbol(Symbol::Pow))
                            } else {
                                res.push(Token::Symbol(Symbol::Mul))
                            }
                        }
                        '/' => res.push(Token::Symbol(Symbol::Div)),
                        '|' => res.push(Token::Symbol(Symbol::InnerDiv)),
                        '^' => res.push(Token::Symbol(Symbol::Pow)),
                        _ => return Err(format!("Unexpected character '{}'", ch))?,
                    }
                }
            }
            None => return Ok(res),
        }
    }
}
