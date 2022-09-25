use std::fmt::Write;

use crate::conditional::BuildCondition;
use crate::{conditional, value, DBImpl};

/**
SQL representation of the DELETE operation.
*/
pub struct SQLDelete<'until_build, 'post_query> {
    pub(crate) dialect: DBImpl,
    pub(crate) model: &'until_build str,
    pub(crate) lookup: Vec<value::Value<'post_query>>,
    pub(crate) where_clause: Option<&'until_build conditional::Condition<'post_query>>,
    pub(crate) limit: Option<u64>,
}

impl<'until_build, 'post_query> SQLDelete<'until_build, 'post_query> {
    /**
    Sets the limit of the delete operation.
    */
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /**
    Adds the a [conditional::Condition] to the delete query.
    */
    pub fn where_clause(
        mut self,
        condition: &'until_build conditional::Condition<'post_query>,
    ) -> Self {
        self.where_clause = Some(condition);
        self
    }

    /**
    Build the DELETE operation
    */
    pub fn build(mut self) -> (String, Vec<value::Value<'post_query>>) {
        return match self.dialect {
            DBImpl::SQLite => {
                let mut s = String::from("DELETE FROM ");
                write!(s, "{} ", self.model).unwrap();
                if self.where_clause.is_some() {
                    write!(
                        s,
                        "WHERE {} ",
                        self.where_clause.unwrap().build(&mut self.lookup)
                    )
                    .unwrap();
                }
                if self.limit.is_some() {
                    write!(s, "LIMIT {}", self.limit.unwrap()).unwrap();
                }
                write!(s, ";").unwrap();
                (s, self.lookup)
            }
            _ => todo!("Not implemented yet!"),
        };
    }
}
