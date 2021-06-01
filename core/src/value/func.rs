use super::{Value, ValueTrait};
use std::fmt;

#[derive(Clone)]
pub(crate) struct Func {
    name: &'static str,
    f: fn(Box<dyn ValueTrait + '_>) -> Result<Value<'static>, String>,
}

impl fmt::Debug for Func {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl ValueTrait for Func {
    fn type_name(&self) -> &'static str {
        "function"
    }

    fn format(&self, _indent: usize, spans: &mut Vec<crate::Span>) {
        spans.push(crate::Span {
            string: self.name.to_string(),
            kind: crate::SpanKind::BuiltInFunction,
        });
    }

    fn apply(&self, arg: Value<'_>) -> Option<Result<Value<'static>, String>> {
        let dyn_val = match arg.expect_dyn() {
            Ok(v) => v,
            Err(msg) => return Some(Err(msg)),
        };
        let res = match (self.f)(dyn_val) {
            Ok(v) => v,
            Err(msg) => return Some(Err(msg)),
        };
        Some(Ok(res))
    }
}

pub(crate) const NOT: Func = Func {
    name: "not",
    f: |val| Ok((!val.as_bool()?).into()),
};