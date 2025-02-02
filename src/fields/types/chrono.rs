use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rorm_db::sql::value::NullType;

use crate::conditions::Value;
use crate::{impl_FieldEq, impl_FieldMin_FieldMax, impl_FieldOrd, impl_FieldType};

impl_FieldType!(NaiveTime, ChronoNaiveTime, Value::ChronoNaiveTime);
impl_FieldEq!(impl<'rhs> FieldEq<'rhs, NaiveTime> for NaiveTime { Value::ChronoNaiveTime });
impl_FieldEq!(impl<'rhs> FieldEq<'rhs, Option<NaiveTime>> for Option<NaiveTime> { |option: Self| option.map(Value::ChronoNaiveTime).unwrap_or(Value::Null(NullType::ChronoNaiveTime)) });
impl_FieldOrd!(NaiveTime, NaiveTime, Value::ChronoNaiveTime);
impl_FieldOrd!(Option<NaiveTime>, Option<NaiveTime>, |option: Self| option
    .map(Value::ChronoNaiveTime)
    .unwrap_or(Value::Null(NullType::ChronoNaiveTime)));
impl_FieldMin_FieldMax!(NaiveTime);

impl_FieldType!(NaiveDate, ChronoNaiveDate, Value::ChronoNaiveDate);
impl_FieldEq!(impl<'rhs> FieldEq<'rhs, NaiveDate> for NaiveDate { Value::ChronoNaiveDate });
impl_FieldEq!(impl<'rhs> FieldEq<'rhs, Option<NaiveDate>> for Option<NaiveDate> { |option: Self| option.map(Value::ChronoNaiveDate).unwrap_or(Value::Null(NullType::ChronoNaiveDate)) });
impl_FieldOrd!(NaiveDate, NaiveDate, Value::ChronoNaiveDate);
impl_FieldOrd!(Option<NaiveDate>, Option<NaiveDate>, |option: Self| option
    .map(Value::ChronoNaiveDate)
    .unwrap_or(Value::Null(NullType::ChronoNaiveDate)));
impl_FieldMin_FieldMax!(NaiveDate);

impl_FieldType!(
    NaiveDateTime,
    ChronoNaiveDateTime,
    Value::ChronoNaiveDateTime
);
impl_FieldEq!(impl<'rhs> FieldEq<'rhs, NaiveDateTime> for NaiveDateTime { Value::ChronoNaiveDateTime });
impl_FieldEq!(impl<'rhs> FieldEq<'rhs, Option<NaiveDateTime>> for Option<NaiveDateTime> { |option: Self| option.map(Value::ChronoNaiveDateTime).unwrap_or(Value::Null(NullType::ChronoNaiveDateTime)) });
impl_FieldOrd!(NaiveDateTime, NaiveDateTime, Value::ChronoNaiveDateTime);
impl_FieldOrd!(
    Option<NaiveDateTime>,
    Option<NaiveDateTime>,
    |option: Self| option
        .map(Value::ChronoNaiveDateTime)
        .unwrap_or(Value::Null(NullType::ChronoNaiveDateTime))
);
impl_FieldMin_FieldMax!(NaiveDateTime);

impl_FieldType!(DateTime<Utc>, ChronoDateTime, Value::ChronoDateTime);
impl_FieldEq!(impl<'rhs> FieldEq<'rhs, DateTime<Utc>> for DateTime<Utc> { Value::ChronoDateTime });
impl_FieldEq!(impl<'rhs> FieldEq<'rhs, Option<DateTime<Utc>>> for Option<DateTime<Utc>> { |option: Self| option.map(Value::ChronoDateTime).unwrap_or(Value::Null(NullType::ChronoDateTime)) });
impl_FieldOrd!(DateTime<Utc>, DateTime<Utc>, Value::ChronoDateTime);
impl_FieldOrd!(
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    |option: Self| option
        .map(Value::ChronoDateTime)
        .unwrap_or(Value::Null(NullType::ChronoDateTime))
);
impl_FieldMin_FieldMax!(DateTime<Utc>);
