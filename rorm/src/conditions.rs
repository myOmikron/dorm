use rorm_declaration::hmr::db_type::{
    Date, DateTime, DbType, Double, Float, Int16, Int32, Int64, Time, VarBinary, VarChar,
};

use crate::conditional::*;
use crate::internal::field::{Field, FieldProxy};
use crate::value::Value;

/// Trait for converting rust values into [`Condition::Value`]'s
pub trait IntoCondValue<'a, D: DbType>: 'a {
    fn into_value(self) -> Value<'a>;
}

impl<'a, S: AsRef<str> + ?Sized> IntoCondValue<'a, VarChar> for &'a S {
    fn into_value(self) -> Value<'a> {
        Value::String(self.as_ref())
    }
}

impl<'a, S: AsRef<[u8]> + ?Sized> IntoCondValue<'a, VarBinary> for &'a S {
    fn into_value(self) -> Value<'a> {
        Value::Binary(self.as_ref())
    }
}

impl<'a, F: Field> IntoCondValue<'a, F::DbType> for &'static FieldProxy<F, ()> {
    fn into_value(self) -> Value<'a> {
        Value::Ident(F::NAME)
    }
}

macro_rules! impl_numeric {
    ($type:ty, $value_variant:ident, $db_type:ident) => {
        impl<'a> IntoCondValue<'a, $db_type> for $type {
            fn into_value(self) -> Value<'a> {
                Value::$value_variant(self)
            }
        }
    };
}
impl_numeric!(i16, I16, Int16);
impl_numeric!(i32, I32, Int32);
impl_numeric!(i64, I64, Int64);
impl_numeric!(f32, F32, Float);
impl_numeric!(f64, F64, Double);
impl_numeric!(chrono::NaiveDate, NaiveDate, Date);
impl_numeric!(chrono::NaiveDateTime, NaiveDateTime, DateTime);
impl_numeric!(chrono::NaiveTime, NaiveTime, Time);

// Helper methods hiding most of the verbosity in creating Conditions
impl<F: Field> FieldProxy<F, ()> {
    fn __column(&self) -> Condition<'static> {
        Condition::Value(Value::Ident(F::NAME))
    }

    fn __unary<'a>(
        &self,
        variant: impl Fn(Box<Condition<'a>>) -> UnaryCondition<'a>,
    ) -> Condition<'a> {
        Condition::UnaryCondition(variant(Box::new(self.__column())))
    }

    fn __binary<'a>(
        &self,
        variant: impl Fn(Box<[Condition<'a>; 2]>) -> BinaryCondition<'a>,
        value: Value<'a>,
    ) -> Condition<'a> {
        Condition::BinaryCondition(variant(Box::new([
            self.__column(),
            Condition::Value(value),
        ])))
    }

    fn __ternary<'a>(
        &self,
        variant: impl Fn(Box<[Condition<'a>; 3]>) -> TernaryCondition<'a>,
        middle: Value<'a>,
        right: Value<'a>,
    ) -> Condition<'a> {
        Condition::TernaryCondition(variant(Box::new([
            self.__column(),
            Condition::Value(middle),
            Condition::Value(right),
        ])))
    }
}

impl<F: Field> FieldProxy<F, ()> {
    /// Check if this field's value lies between two other values
    pub fn between<'a>(
        &self,
        lower: impl IntoCondValue<'a, F::DbType>,
        upper: impl IntoCondValue<'a, F::DbType>,
    ) -> Condition<'a> {
        self.__ternary(
            TernaryCondition::Between,
            lower.into_value(),
            upper.into_value(),
        )
    }

    /// Check if this field's value does not lie between two other values
    pub fn not_between<'a>(
        &self,
        lower: impl IntoCondValue<'a, F::DbType>,
        upper: impl IntoCondValue<'a, F::DbType>,
    ) -> Condition<'a> {
        self.__ternary(
            TernaryCondition::NotBetween,
            lower.into_value(),
            upper.into_value(),
        )
    }

    /// Check if this field's value is equal to another value
    pub fn equals<'a>(&self, value: impl IntoCondValue<'a, F::DbType>) -> Condition<'a> {
        self.__binary(BinaryCondition::Equals, value.into_value())
    }

    /// Check if this field's value is not equal to another value
    pub fn not_equals<'a>(&self, value: impl IntoCondValue<'a, F::DbType>) -> Condition<'a> {
        self.__binary(BinaryCondition::NotEquals, value.into_value())
    }

    /// Check if this field's value is greater than another value
    pub fn greater<'a>(&self, value: impl IntoCondValue<'a, F::DbType>) -> Condition<'a> {
        self.__binary(BinaryCondition::Greater, value.into_value())
    }

    /// Check if this field's value is greater than or equal to another value
    pub fn greater_or_equals<'a>(&self, value: impl IntoCondValue<'a, F::DbType>) -> Condition<'a> {
        self.__binary(BinaryCondition::GreaterOrEquals, value.into_value())
    }

    /// Check if this field's value is less than another value
    pub fn less<'a>(&self, value: impl IntoCondValue<'a, F::DbType>) -> Condition<'a> {
        self.__binary(BinaryCondition::Less, value.into_value())
    }

    /// Check if this field's value is less than or equal to another value
    pub fn less_or_equals<'a>(&self, value: impl IntoCondValue<'a, F::DbType>) -> Condition<'a> {
        self.__binary(BinaryCondition::LessOrEquals, value.into_value())
    }

    /// Check if this field's value is similar to another value
    pub fn like<'a>(&self, value: impl IntoCondValue<'a, F::DbType>) -> Condition<'a> {
        self.__binary(BinaryCondition::Like, value.into_value())
    }

    /// Check if this field's value is not similar to another value
    pub fn not_like<'a>(&self, value: impl IntoCondValue<'a, F::DbType>) -> Condition<'a> {
        self.__binary(BinaryCondition::NotLike, value.into_value())
    }

    /// Check if this field's value is matched by a regex
    pub fn regexp<'a>(&self, value: impl IntoCondValue<'a, F::DbType>) -> Condition<'a> {
        self.__binary(BinaryCondition::Regexp, value.into_value())
    }

    /// Check if this field's value is not matched by a regex
    pub fn not_regexp<'a>(&self, value: impl IntoCondValue<'a, F::DbType>) -> Condition<'a> {
        self.__binary(BinaryCondition::NotRegexp, value.into_value())
    }

    // TODO in, not_in: requires different trait than IntoCondValue
    // TODO is_null, is_not_null: check AsDbType::NULLABLE in type constraint, new Nullable trait?
}
