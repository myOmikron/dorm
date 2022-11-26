//! Query builder and macro
use std::marker::PhantomData;
use std::ops::{Range, RangeInclusive, Sub};

use crate::conditions::Condition;
use futures::{Stream, StreamExt};
use rorm_db::database::ColumnSelector;
use rorm_db::limit_clause::LimitClause;
use rorm_db::transaction::Transaction;
use rorm_db::{error::Error, row::Row, Database};

use crate::crud::builder::{ConditionMarker, TransactionMarker};
use crate::internal::field::as_db_type::AsDbType;
use crate::internal::field::{Field, FieldProxy};
use crate::internal::query_context::{QueryContext, QueryContextBuilder};
use crate::internal::relation_path::Path;
use crate::model::{Model, Patch};
use crate::sealed;

/// Builder for select queries
///
/// Is is recommended to start a builder using [query!].
///
/// [query!]: macro@crate::query
#[must_use]
pub struct QueryBuilder<
    'db,
    'rf,
    M: Model,
    S: Selector<M>,
    C: ConditionMarker<'rf>,
    T: TransactionMarker<'rf, 'db>,
    LO: LimOffMarker,
> {
    db: &'db Database,
    selector: S,
    _phantom: PhantomData<&'rf M>,

    condition: C,
    transaction: T,

    lim_off: LO,
}

/// Specifies which columns to query and how to decode the rows into what.
pub trait Selector<M: Model> {
    /// Type as which rows should be decoded
    type Result;

    /// Decode a row
    fn decode(row: Row) -> Result<Self::Result, Error>;

    /// Columns to query
    fn columns(&self, builder: &mut QueryContextBuilder) -> &[ColumnSelector<'static>];
}

impl<'db, 'rf, M: Model, S: Selector<M>> QueryBuilder<'db, 'rf, M, S, (), (), ()> {
    /// Start building a query using a generic [Selector]
    pub fn new(db: &'db Database, selector: S) -> Self {
        QueryBuilder {
            db,
            selector,
            _phantom: PhantomData,

            condition: (),
            transaction: (),

            lim_off: (),
        }
    }
}

impl<'db, 'a, M: Model, S: Selector<M>, T: TransactionMarker<'a, 'db>, LO: LimOffMarker>
    QueryBuilder<'db, 'a, M, S, (), T, LO>
{
    /// Add a condition to the query
    pub fn condition<C: Condition<'a>>(
        self,
        condition: C,
    ) -> QueryBuilder<'db, 'a, M, S, C, T, LO> {
        #[rustfmt::skip]
        let QueryBuilder { db, selector, _phantom, transaction, lim_off, .. } = self;
        #[rustfmt::skip]
        return QueryBuilder { db, selector, _phantom, condition, transaction, lim_off, };
    }
}

impl<'db, 'a, M: Model, S: Selector<M>, C: ConditionMarker<'a>, LO: LimOffMarker>
    QueryBuilder<'db, 'a, M, S, C, (), LO>
{
    /// Add a transaction to the query
    pub fn transaction(
        self,
        transaction: &'a mut Transaction<'db>,
    ) -> QueryBuilder<'db, 'a, M, S, C, &'a mut Transaction<'db>, LO> {
        #[rustfmt::skip]
        let QueryBuilder { db, selector, _phantom, condition, lim_off,  .. } = self;
        #[rustfmt::skip]
        return QueryBuilder { db, selector, _phantom, condition, transaction, lim_off, };
    }
}

impl<
        'db,
        'a,
        M: Model,
        S: Selector<M>,
        C: ConditionMarker<'a>,
        T: TransactionMarker<'a, 'db>,
        O: OffsetMarker,
    > QueryBuilder<'db, 'a, M, S, C, T, O>
{
    /// Add a limit to the query
    pub fn limit(self, limit: u64) -> QueryBuilder<'db, 'a, M, S, C, T, Limit<O>> {
        #[rustfmt::skip]
        let QueryBuilder { db, selector, _phantom, condition, transaction,  lim_off } = self;
        #[rustfmt::skip]
        return QueryBuilder { db, selector, _phantom, condition, transaction, lim_off: Limit { limit, offset: lim_off }, };
    }
}

impl<
        'db,
        'a,
        M: Model,
        S: Selector<M>,
        C: ConditionMarker<'a>,
        T: TransactionMarker<'a, 'db>,
        LO: AcceptsOffset,
    > QueryBuilder<'db, 'a, M, S, C, T, LO>
{
    /// Add a offset to the query
    pub fn offset(self, offset: u64) -> QueryBuilder<'db, 'a, M, S, C, T, LO::Result> {
        #[rustfmt::skip]
        let QueryBuilder { db, selector, _phantom, condition, transaction, lim_off,  .. } = self;
        let lim_off = lim_off.add_offset(offset);
        #[rustfmt::skip]
        return QueryBuilder { db, selector, _phantom, condition, transaction, lim_off};
    }
}

impl<'db, 'a, M: Model, S: Selector<M>, C: ConditionMarker<'a>, T: TransactionMarker<'a, 'db>>
    QueryBuilder<'db, 'a, M, S, C, T, ()>
{
    /// Add a offset to the query
    pub fn range(
        self,
        range: impl FiniteRange<u64>,
    ) -> QueryBuilder<'db, 'a, M, S, C, T, Limit<u64>> {
        #[rustfmt::skip]
        let QueryBuilder { db, selector, _phantom, condition, transaction,  .. } = self;
        let limit = Limit {
            limit: range.len(),
            offset: range.start(),
        };
        #[rustfmt::skip]
        return QueryBuilder { db, selector, _phantom, condition, transaction, lim_off: limit, };
    }
}

impl<
        'rf,
        'db: 'rf,
        M: Model,
        S: Selector<M>,
        C: ConditionMarker<'rf>,
        T: TransactionMarker<'rf, 'db>,
        L: LimitMarker,
    > QueryBuilder<'db, 'rf, M, S, C, T, L>
{
    /// Retrieve and decode all matching rows
    pub async fn all(self) -> Result<Vec<S::Result>, Error> {
        let mut builder = QueryContextBuilder::new();
        let columns = self.selector.columns(&mut builder);
        self.condition.add_to_builder(&mut builder);
        let context = builder.finish();
        let joins = context.get_joins();
        self.db
            .query_all(
                M::TABLE,
                columns,
                &joins,
                self.condition.into_option(&context).as_ref(),
                &[],
                self.lim_off.into_option(),
                self.transaction.into_option(),
            )
            .await?
            .into_iter()
            .map(|x| {
                S::decode(x).map_err(|_| Error::DecodeError("Could not decode row".to_string()))
            })
            .collect::<Result<Vec<_>, _>>()
    }

    /// Retrieve and decode the query as a stream
    ///
    /// In order to resolve joins a [QueryContext] is needed, which has to survive the query.
    /// So as temporary solution it has to be provided in order to stream a query.
    /// This is **WIP**!
    pub fn stream(
        self,
        context: &'rf mut QueryContext, // TODO
    ) -> impl Stream<Item = Result<S::Result, Error>> + 'rf {
        let mut builder = QueryContextBuilder::new();
        let columns = self.selector.columns(&mut builder);
        self.condition.add_to_builder(&mut builder);
        *context = builder.finish();
        let joins = context.get_joins();
        self.db
            .query_stream(
                M::TABLE,
                columns,
                &joins,
                self.condition.into_option(context).as_ref(),
                &[],
                self.lim_off.into_option(),
                self.transaction.into_option(),
            )
            .map(|row| {
                S::decode(row?).map_err(|_| Error::DecodeError("Could not decode row".to_string()))
            })
    }
}

impl<
        'rf,
        'db: 'rf,
        M: Model,
        S: Selector<M>,
        C: ConditionMarker<'rf>,
        T: TransactionMarker<'rf, 'db>,
        O: OffsetMarker,
    > QueryBuilder<'db, 'rf, M, S, C, T, O>
{
    /// Retrieve and decode exactly one matching row
    ///
    /// An error is returned if no value could be retrieved.
    pub async fn one(self) -> Result<S::Result, Error> {
        let mut builder = QueryContextBuilder::new();
        let columns = self.selector.columns(&mut builder);
        self.condition.add_to_builder(&mut builder);
        let context = builder.finish();
        let joins = context.get_joins();
        let row = self
            .db
            .query_one(
                M::TABLE,
                columns,
                &joins,
                self.condition.into_option(&context).as_ref(),
                &[],
                self.lim_off.into_option(),
                self.transaction.into_option(),
            )
            .await?;
        S::decode(row).map_err(|_| Error::DecodeError("Could not decode row".to_string()))
    }

    /// Try to retrieve and decode a matching row
    pub async fn optional(self) -> Result<Option<S::Result>, Error> {
        let mut builder = QueryContextBuilder::new();
        let columns = self.selector.columns(&mut builder);
        self.condition.add_to_builder(&mut builder);
        let context = builder.finish();
        let joins = context.get_joins();
        let row = self
            .db
            .query_optional(
                M::TABLE,
                columns,
                &joins,
                self.condition.into_option(&context).as_ref(),
                &[],
                self.lim_off.into_option(),
                self.transaction.into_option(),
            )
            .await?;
        match row {
            None => Ok(None),
            Some(row) => {
                Ok(Some(S::decode(row).map_err(|_| {
                    Error::DecodeError("Could not decode row".to_string())
                })?))
            }
        }
    }
}

/// Create a SELECT query.
///
/// 1. Give a reference to your db and the patch to query.
///     If you just need a few fields and don't want to create a patch for it,
///     you can specify these fields directly as a tuple as well.
///
///     `query!(&db, MyModelType)`
///
///     `query!(&db, (MyModelType::F.some_field, MyModelType::F.another_field, ))`
///
/// 2. Set a condition which rows to query.
///
///     `.condition(MyModelType::F.some_field.equals("some_value"))`
///
/// 3. *Optionally* add a limit or offset to restrict your query size.
///
///     `.limit(5)`
///
///     `.offset(2)`
///
///     `.range(2..7)`
///
/// 4. *Optionally* add this query to a transaction
///
///     `.transaction(&mut tr)`
///
/// 5. Finally specify how to get the queries results. This will also execute the query.
///     - Get [`all`](QueryBuilder::all) matching rows in a vector.
///
///         `.all().await`
///
///     - Get all matching rows in an async [`stream`](QueryBuilder::stream).
///
///         `.stream()`
///
///     - Just get exactly [`one`](QueryBuilder::one) row.
///
///         `.one().await`
///
///     - Get one row if any. ([`optional`](QueryBuilder::optional))
///
///         `.optional().await`
///
///     Each of these methods decodes the database's rows into the patch you specified in step 1.
///     If you want to work with raw rows, each of the methods in step 4 has a `*_as_row` twin.
///
/// Example:
/// ```no_run
/// # use rorm::{Model, Database, query};
/// #
/// # #[derive(Model)]
/// # struct User {
/// #     #[rorm(id)]
/// #     id: i64,
/// #     username: String,
/// #     password: String,
/// # }
/// #
/// #
/// # async fn shame_user(_user: &User) {}
/// #
/// pub async fn shame_users(db: &Database) {
///     for (id, password) in query!(db, (User::F.id, User::F.password)).all().await.unwrap() {
///         if password == "password" {
///             let user = query!(db, User)
///                 .condition(User::F.id.equals(id))
///                 .one()
///                 .await
///                 .unwrap();
///             shame_user(&user).await;
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! query {
    (replace $anything:tt with $result:tt) => { $result };
    ($db:expr, $patch:path) => {
        $crate::crud::query::QueryBuilder::new(
            $db,
            $crate::crud::query::SelectPatch::<$patch>::new(),
        )
    };
    ($db:expr, ($($field:expr),+$(,)?)) => {
        $crate::crud::query::QueryBuilder::new(
            $db,
            $crate::crud::query::SelectTuple::<_, { 0 $( + $crate::query!(replace {$field} with 1))+ }>::new(($($field,)+)),
        )
    };
}

/// Finite alternative to [RangeBounds](std::ops::RangeBounds)
///
/// It unifies [Range] and [RangeInclusive]
#[allow(clippy::len_without_is_empty)] // Since it is generic, there is no trivial way to compare with zero
pub trait FiniteRange<T> {
    // and I don't see why I should use an "IsZero" trait, just to satisfy clippy.
    /// The lower bound of the range (inclusive)
    fn start(&self) -> T;

    /// The upper bound of the range (exclusive)
    fn end(&self) -> T;

    /// The number of items contained in this range
    fn len(&self) -> T
    where
        T: Sub<T, Output = T> + Copy,
    {
        self.end() - self.start()
    }
}
impl<T: Copy> FiniteRange<T> for Range<T> {
    fn start(&self) -> T {
        self.start
    }

    fn end(&self) -> T {
        self.end
    }
}
impl FiniteRange<u64> for RangeInclusive<u64> {
    fn start(&self) -> u64 {
        *self.start()
    }

    fn end(&self) -> u64 {
        *self.end() + 1
    }
}

/// Unification of [LimitMarker] and [OffsetMarker]
pub trait LimOffMarker {
    sealed!();
}
impl LimOffMarker for () {}
impl<O: OffsetMarker> LimOffMarker for Limit<O> {}
impl LimOffMarker for u64 {}

/// Marker for the generic parameter storing a limit.
///
/// Valid values are `()`, `Limit<()>` and `Limit<u64>`.
pub trait LimitMarker: LimOffMarker {
    sealed!();

    /// Convert the generic limit into [Option<LimitClause>]
    fn into_option(self) -> Option<LimitClause>;
}
impl LimitMarker for () {
    fn into_option(self) -> Option<LimitClause> {
        None
    }
}
/// A query limit and optional offset
pub struct Limit<O: OffsetMarker> {
    /// Number of rows to query
    limit: u64,

    /// Optional offset to begin query at
    offset: O,
}
impl<O: OffsetMarker> LimitMarker for Limit<O> {
    fn into_option(self) -> Option<LimitClause> {
        Some(LimitClause {
            limit: self.limit,
            offset: self.offset.into_option(),
        })
    }
}

/// Unification of `()` and `Limit<()>`
pub trait AcceptsOffset: LimOffMarker {
    sealed!();

    /// The resulting type i.e. `u64` or `Limit<u64>`
    type Result: LimOffMarker;
    /// "Add" the offset to the type
    fn add_offset(self, offset: u64) -> Self::Result;
}
impl AcceptsOffset for () {
    type Result = u64;
    fn add_offset(self, offset: u64) -> Self::Result {
        offset
    }
}
impl AcceptsOffset for Limit<()> {
    type Result = Limit<u64>;
    fn add_offset(self, offset: u64) -> Self::Result {
        let Limit { limit, offset: _ } = self;
        Limit { limit, offset }
    }
}

/// Marker for the generic parameter storing a limit's offset.
///
/// Valid values are `()` and `u64`.
pub trait OffsetMarker: LimOffMarker {
    sealed!();

    /// Convert the generic offset into [Option<u64>]
    fn into_option(self) -> Option<u64>;
}
impl OffsetMarker for () {
    fn into_option(self) -> Option<u64> {
        None
    }
}
impl OffsetMarker for u64 {
    fn into_option(self) -> Option<u64> {
        Some(self)
    }
}

/// The [Selector] for patches.
pub struct SelectPatch<P: Patch> {
    patch: PhantomData<P>,
    columns: Vec<ColumnSelector<'static>>,
}
impl<P: Patch> SelectPatch<P> {
    /// Create a SelectPatch
    pub fn new() -> Self {
        SelectPatch {
            patch: PhantomData,
            columns: P::COLUMNS
                .iter()
                .flatten()
                .map(|x| ColumnSelector {
                    table_name: Some(P::Model::TABLE),
                    column_name: x,
                    select_alias: None,
                    aggregation: None,
                })
                .collect(),
        }
    }
}
impl<P: Patch> Default for SelectPatch<P> {
    fn default() -> Self {
        Self::new()
    }
}
impl<M: Model, P: Patch<Model = M>> Selector<M> for SelectPatch<P> {
    type Result = P;

    fn decode(row: Row) -> Result<Self::Result, Error> {
        P::from_row(row)
    }

    fn columns(&self, _builder: &mut QueryContextBuilder) -> &[ColumnSelector<'static>] {
        &self.columns
    }
}

/// The [Selector] for tuple
///
/// Implemented for tuple of size 8 or less.
pub struct SelectTuple<T, const C: usize> {
    #[allow(dead_code)]
    tuple: T,
    columns: [ColumnSelector<'static>; C],
}
macro_rules! impl_select_tuple {
    ($C:literal, ($($index:tt: <$F:ident, $P:ident>,)+)) => {
        impl<$($F: Field, $P: Path),+> SelectTuple<($(FieldProxy<$F, $P>,)+), $C> {
            /// Create a SelectTuple
            pub const fn new(tuple: ($(FieldProxy<$F, $P>,)+)) -> Self {
                Self {
                    tuple,
                    columns: [$(
                        ColumnSelector {
                            table_name: Some($P::ALIAS),
                            column_name: $F::NAME,
                            select_alias: None,
                            aggregation: None,
                        }
                    ),+],
                }
            }
        }
        impl<M: Model, $($F: Field, $P: Path<Origin = M>),+> Selector<M>
            for SelectTuple<($(FieldProxy<$F, $P>,)+), $C>
        {
            type Result = ($($F::Type,)+);

            fn decode(row: Row) -> Result<Self::Result, Error> {
                Ok(($($F::Type::from_primitive(row.get::<<<$F as Field>::Type as AsDbType>::Primitive, usize>($index)?),)+))
            }

            fn columns(&self, builder: &mut QueryContextBuilder) -> &[ColumnSelector<'static>] {
                $($P::add_to_join_builder(builder);)+
                &self.columns
            }
        }
    };
}
impl_select_tuple!(1, (0: <F0, P0>,));
impl_select_tuple!(2, (0: <F0, P0>, 1: <F1, P1>,));
impl_select_tuple!(3, (0: <F0, P0>, 1: <F1, P1>, 2: <F2, P2>,));
impl_select_tuple!(4, (0: <F0, P0>, 1: <F1, P1>, 2: <F2, P2>, 3: <F3, P3>,));
impl_select_tuple!(5, (0: <F0, P0>, 1: <F1, P1>, 2: <F2, P2>, 3: <F3, P3>, 4: <F4, P4>,));
impl_select_tuple!(6, (0: <F0, P0>, 1: <F1, P1>, 2: <F2, P2>, 3: <F3, P3>, 4: <F4, P4>, 5: <F5, P5>,));
impl_select_tuple!(7, (0: <F0, P0>, 1: <F1, P1>, 2: <F2, P2>, 3: <F3, P3>, 4: <F4, P4>, 5: <F5, P5>, 6: <F6, P6>,));
impl_select_tuple!(8, (0: <F0, P0>, 1: <F1, P1>, 2: <F2, P2>, 3: <F3, P3>, 4: <F4, P4>, 5: <F5, P5>, 6: <F6, P6>, 7: <F7, P7>,));
