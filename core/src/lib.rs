#![forbid(unsafe_code)]
#![forbid(clippy::all)]
#![allow(clippy::try_err)] // allow `Err(..)?`
#![allow(clippy::too_many_arguments)] // format_trailing_digits in BigRat
#![deny(clippy::pedantic)]
#![allow(clippy::too_many_lines)] // BigRat::format
#![allow(clippy::non_ascii_literal)]
#![doc(html_root_url = "https://docs.rs/fend-core/0.1.4")]

mod ast;
mod err;
mod eval;
mod interrupt;
mod lexer;
mod num;
mod parser;
mod scope;
mod value;

pub use interrupt::Interrupt;

/// This contains the result of a computation.
#[derive(PartialEq, Eq, Debug)]
pub struct FendResult {
    main_result: String,
    other_info: Vec<String>,
}

impl FendResult {
    /// This retrieves the main result of the computation.
    #[must_use]
    pub fn get_main_result(&self) -> &str {
        self.main_result.as_str()
    }

    /// This retrieves a list of other results of the computation. It is less
    /// stable than the main result, and should only be shown for when used
    /// interactively.
    pub fn get_other_info(&self) -> impl Iterator<Item = &str> {
        self.other_info.iter().map(std::string::String::as_str)
    }
}

/// This struct contains context used for `fend`. It should only be created once
/// at startup.
#[derive(Clone)]
pub struct Context {
    scope: scope::Scope,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    /// Create a new context instance. This can be fairly slow, and should
    /// only be done once if possible.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scope: scope::Scope::new_default(&crate::interrupt::Never::default()).unwrap(),
        }
    }
}

/// This function evaluates a string using the given context. Any evaluation using this
/// function cannot be interrupted.
///
/// For example, passing in the string `"1 + 1"` will return a result of `"2"`.
///
/// # Errors
/// It returns an error if the given string is invalid.
/// This may be due to parser or runtime errors.
pub fn evaluate(input: &str, context: &mut Context) -> Result<FendResult, String> {
    evaluate_with_interrupt(input, context, &interrupt::Never::default())
}

/// This function evaluates a string using the given context and the provided
/// Interrupt object.
///
/// For example, passing in the string `"1 + 1"` will return a result of `"2"`.
///
/// # Errors
/// It returns an error if the given string is invalid.
/// This may be due to parser or runtime errors.
pub fn evaluate_with_interrupt(
    input: &str,
    context: &mut Context,
    int: &impl Interrupt,
) -> Result<FendResult, String> {
    if input.is_empty() {
        // no or blank input: return no output
        return Ok(FendResult {
            main_result: "".to_string(),
            other_info: vec![],
        });
    }
    let result = match eval::evaluate_to_string(input, &mut context.scope, int) {
        Ok(value) => value,
        // TODO: handle different interrupt values
        Err(err::IntErr::Interrupt(_)) => return Err("Interrupted".to_string()),
        Err(err::IntErr::Error(e)) => return Err(e),
    };
    Ok(FendResult {
        main_result: result,
        other_info: vec![],
    })
}

/// Returns the current version of `fend-core`.
#[must_use]
pub fn get_version() -> String {
    "0.1.4".to_string()
}

/// Returns the current extended version of `fend-core`, which includes the
/// release date in addition to the semver number
#[must_use]
pub fn get_extended_version() -> String {
    "0.1.4 (2020-09-15)".to_string()
}
