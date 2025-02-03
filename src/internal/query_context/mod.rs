//! The query context holds some of a query's data which rorm-db borrows.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write;
use std::ops::{Deref, DerefMut};

use rorm_db::sql::join_table::JoinType;
use rorm_db::sql::ordering::Ordering;

use crate::conditions::{BinaryOperator, Condition, Value};
use crate::crud::selector::AggregatedColumn;
use crate::fields::proxy::FieldProxyImpl;
use crate::internal::field::Field;
use crate::internal::query_context::flat_conditions::{FlatCondition, GetConditionError};
use crate::internal::query_context::ids::PathId;
use crate::internal::relation_path::{Path, PathField};
use crate::Model;

pub mod flat_conditions;
pub mod ids;

/// Context for creating queries.
///
/// Since rorm-db borrows all of its parameters, there has to be someone who own it.
/// This struct owns all the implicit data required to query something i.e. join and alias information.
#[derive(Debug, Default)]
pub struct QueryContext<'v> {
    join_aliases: HashMap<PathId, String>,
    selects: Vec<Select>,
    joins: Vec<Join>,
    order_bys: Vec<OrderBy>,
    pub(crate) conditions: Vec<FlatCondition>,
    pub(crate) values: Vec<Value<'v>>,
}
impl<'v> QueryContext<'v> {
    /// Create an empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a field to select returning its index and alias
    pub fn select_field<F: Field, P: Path>(&mut self) -> (usize, String) {
        P::add_to_context(self);
        let alias = format!("{}", NumberAsAZ(self.selects.len()));
        self.selects.push(Select {
            table_name: PathId::of::<P>(),
            column_name: F::NAME,
            select_alias: alias.clone(),
            aggregation: None,
        });
        (self.selects.len() - 1, alias)
    }

    /// Add a field to aggregate returning its index and alias
    pub fn select_aggregation<I: FieldProxyImpl, R>(
        &mut self,
        column: AggregatedColumn<I, R>,
    ) -> (usize, String) {
        I::Path::add_to_context(self);
        let alias = format!("{}", NumberAsAZ(self.selects.len()));
        self.selects.push(Select {
            table_name: PathId::of::<I::Path>(),
            column_name: I::Field::NAME,
            select_alias: alias.clone(),
            aggregation: Some(column.sql),
        });
        (self.selects.len() - 1, alias)
    }

    /// Adds a condition to the query context and returns its index
    /// which can be used to retrieve it `rorm-sql` representation.
    ///
    /// (Use the index with [`QueryContext::get_condition`])
    pub fn add_condition(&mut self, condition: &(impl Condition<'v> + ?Sized)) -> usize {
        let index = self.conditions.len();
        condition.build(self);
        index
    }

    /// Add a field to order by
    pub fn order_by_field<F: Field, P: Path>(&mut self, ordering: Ordering) {
        P::add_to_context(self);
        self.order_bys.push(OrderBy {
            column_name: F::NAME,
            table_name: PathId::of::<P>(),
            ordering,
        })
    }

    /// Create a vector borrowing the joins in rorm_db's format which can be passed to it as slice.
    pub fn get_joins(&self) -> Vec<rorm_db::database::JoinTable> {
        self.joins
            .iter()
            .map(
                |Join {
                     table_name,
                     join_alias,
                     join_condition,
                 }| rorm_db::database::JoinTable {
                    join_type: JoinType::Join,
                    table_name,
                    join_alias: self.join_aliases.get(join_alias).unwrap(),
                    join_condition: Cow::Owned(self.get_condition(*join_condition)),
                },
            )
            .collect()
    }

    /// Create a vector borrowing the selects in rorm_db's format which can be passed to it as slice.
    pub fn get_selects(&self) -> Vec<rorm_db::database::ColumnSelector> {
        self.selects
            .iter()
            .map(
                |Select {
                     table_name,
                     column_name,
                     select_alias,
                     aggregation,
                 }| {
                    rorm_db::database::ColumnSelector {
                        table_name: Some(self.join_aliases.get(table_name).unwrap()),
                        column_name,
                        select_alias: Some(select_alias.as_str()),
                        aggregation: *aggregation,
                    }
                },
            )
            .collect()
    }

    /// Retrieves the `rorm-sql` representation of a previously added `Condition`.
    ///
    /// # Errors
    /// If the index is invalid (wasn't returned by a previous call to [`QueryContext::add_condition`])
    /// or the `Condition`'s implementation left the query context in an invalid state.
    ///
    /// Since both cases are programmers' faults,
    /// you could consider [`QueryContext::get_condition`] which simply panics.
    pub fn try_get_condition(
        &self,
        index: usize,
    ) -> Result<rorm_db::sql::conditional::Condition, GetConditionError> {
        let (head, mut tail) = self
            .conditions
            .get(index..)
            .and_then(|subslice| {
                let mut nodes = subslice.iter().copied();
                nodes.next().zip(Some(nodes))
            })
            .ok_or(GetConditionError::MissingNodes)?;
        self.get_condition_inner(head, &mut tail)
    }

    /// Retrieves the `rorm-sql` representation of a previously added `Condition`.
    ///
    /// If you want an error instead of panicking, use [`QueryContext::try_get_condition`].
    ///
    /// [`QueryContext::get_condition_opt`] might be a handy shorthand,
    /// when working with `ConditionMarker` or other sources of optional conditions
    ///
    /// # Panics
    /// If the index is invalid (wasn't returned by a previous call to [`QueryContext::add_condition`])
    /// or the `Condition`'s implementation left the query context in an invalid state.
    pub fn get_condition(&self, index: usize) -> rorm_db::sql::conditional::Condition {
        self.try_get_condition(index)
            .expect("Got invalid condition index")
    }

    /// Shorthand for calling [`Self::get_condition`] on an optional index
    pub fn get_condition_opt(
        &self,
        index: Option<usize>,
    ) -> Option<rorm_db::sql::conditional::Condition> {
        index.map(|index| self.get_condition(index))
    }

    /// Create a vector borrowing the order bys in rorm_db's format which can be passed to it as slice.
    pub fn get_order_bys(&self) -> Vec<rorm_db::sql::ordering::OrderByEntry> {
        self.order_bys
            .iter()
            .map(|order_by| rorm_db::sql::ordering::OrderByEntry {
                ordering: order_by.ordering,
                table_name: Some(self.join_aliases.get(&order_by.table_name).unwrap()),
                column_name: order_by.column_name,
            })
            .collect()
    }

    /// Create a vector borrowing the selects only by their `column_name` to be used in `INSERT RETURNING`.
    ///
    /// This method also checks, if the context would be valid in the first place.
    pub fn get_returning(&self) -> Option<Vec<&'static str>> {
        // Disallow joins
        if !self.joins.is_empty() {
            return None;
        }

        let mut returning = Vec::with_capacity(self.selects.len());
        let table_name = self.selects.first()?.table_name;
        for select in &self.selects {
            // Disallow aggregation
            if select.aggregation.is_some() {
                return None;
            }

            // Disallow different tables (theoretically unnecessary?)
            if select.table_name != table_name {
                return None;
            }

            returning.push(select.column_name);
        }
        Some(returning)
    }

    /// Creates a temporary scope in which every path used will be implicitly appended to a base path `P`.
    pub fn with_base_path<'ctx, P: Path>(&'ctx mut self) -> WithBasePath<'ctx, 'v> {
        todo!()
    }
}
/// Guard like wrapper for `QueryContext` returned by [`QueryContext::with_base_path`]
pub struct WithBasePath<'ctx, 'v> {
    ctx: &'ctx mut QueryContext<'v>,
}
impl<'ctx, 'v> Drop for WithBasePath<'ctx, 'v> {
    fn drop(&mut self) {
        todo!("Restore ctx to orginal state");
    }
}
impl<'ctx, 'v> Deref for WithBasePath<'ctx, 'v> {
    type Target = QueryContext<'v>;

    fn deref(&self) -> &Self::Target {
        &*self.ctx
    }
}
impl<'ctx, 'v> DerefMut for WithBasePath<'ctx, 'v> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.ctx
    }
}

impl<'v> QueryContext<'v> {
    /// **Use [`Path::add_to_context`], this method is its impl detail!**
    ///
    /// Add the origin model to the builder
    pub(crate) fn add_path_origin<M: Model>(&mut self) -> &'static str {
        self.join_aliases
            .entry(PathId::of::<M>())
            .or_insert_with(|| M::TABLE.to_string());
        M::TABLE
    }

    /// **Use [`Path::add_to_context`], this method is its impl detail!**
    ///
    /// Recursively add a relation path to the builder
    ///
    /// The generic parameters are the parameters defining the outer most [PathStep].
    pub(crate) fn add_path_step<F, P>(&mut self) -> &str
    where
        F: Field + PathField<<F as Field>::Type>,
        P: Path<Current = <F::ParentField as Field>::Model>,
    {
        let path_id = PathId::of::<P::Step<F>>();
        if !self.join_aliases.contains_key(&path_id) {
            P::add_to_context(self);
            let alias = format!("{}", NumberAsAZ(self.join_aliases.len()));
            self.join_aliases.insert(path_id, alias);
            self.joins.push({
                Join {
                    table_name: <<F as PathField<_>>::ChildField as Field>::Model::TABLE,
                    join_alias: path_id,
                    join_condition: self.conditions.len(),
                }
            });
            self.conditions.extend([
                FlatCondition::BinaryCondition(BinaryOperator::Equals),
                FlatCondition::Column(path_id, <F as PathField<_>>::ChildField::NAME),
                FlatCondition::Column(PathId::of::<P>(), <F as PathField<_>>::ParentField::NAME),
            ]);
        }
        self.join_aliases.get(&path_id).unwrap()
    }
}

#[derive(Debug, Clone)]
struct Select {
    table_name: PathId,
    column_name: &'static str,
    select_alias: String,
    aggregation: Option<rorm_db::sql::aggregation::SelectAggregator>,
}

#[derive(Debug, Clone)]
struct Join {
    table_name: &'static str,
    join_alias: PathId,
    join_condition: usize,
}

#[derive(Debug, Clone)]
struct OrderBy {
    column_name: &'static str,
    table_name: PathId,
    ordering: Ordering,
}

/// Adapter to display a number using the alphabet as digits
struct NumberAsAZ(usize);
impl fmt::Display for NumberAsAZ {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        static ALPHABET: [char; 26] = [
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q',
            'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
        ];
        let mut x = self.0;
        match x {
            0..26 => f.write_char(ALPHABET[x]),
            _ => {
                while x > 26 {
                    f.write_char(ALPHABET[x % 26])?;
                    x /= 26;
                }
                f.write_char(ALPHABET[x])
            }
        }
    }
}
