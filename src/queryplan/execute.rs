use columnvalueops::ColumnValueOps;
use databaseinfo::DatabaseInfo;
use databasestorage::DatabaseStorage;
use super::sexpression::{BinaryOp, SExpression};

struct Source<'a, ColumnValue: Sized + 'static> {
    parent: Option<&'a Source<'a, ColumnValue>>,
    source_id: u32,
    row: &'a [ColumnValue],
}

impl<'a, ColumnValue: Sized> Source<'a, ColumnValue> {
    fn find_row_from_source_id(&self, source_id: u32) -> Option<&[ColumnValue]> {
        if self.source_id == source_id {
            Some(self.row)
        } else if let Some(parent) = self.parent {
            parent.find_row_from_source_id(source_id)
        } else {
            None
        }
    }
}

/// The query plan is currently defined as a recursive language.
/// Because of this, it would take some work (and foresight) to make query plan
/// execution co-operate with the concept of iterators.
/// For now, callbacks are used to yield rows.
///
/// TODO: translate query plan into procedural language
/// (such as VM instructions, like those found in SQLite's VBDE).
pub struct ExecuteQueryPlan<'s, Storage: DatabaseStorage + 's> {
    storage: &'s Storage
}

impl<'a, 's, Storage: DatabaseStorage> ExecuteQueryPlan<'s, Storage>
where <Storage::Info as DatabaseInfo>::Table: 'a
{
    pub fn new(storage: &'s Storage) -> ExecuteQueryPlan<'s, Storage> {
        ExecuteQueryPlan {
            storage: storage
        }
    }

    pub fn execute_query_plan<'b, 'c>(&self, expr: &SExpression<'a, Storage::Info>,
    result_cb: &'c mut FnMut(&[<Storage::Info as DatabaseInfo>::ColumnValue]) -> Result<(), ()>)
    -> Result<(), ()>
    {
        self.execute(expr, result_cb, None)
    }

    fn execute<'b, 'c>(&self, expr: &SExpression<'a, Storage::Info>,
        result_cb: &'c mut FnMut(&[<Storage::Info as DatabaseInfo>::ColumnValue]) -> Result<(), ()>,
        source: Option<&Source<'b, <Storage::Info as DatabaseInfo>::ColumnValue>>)
    -> Result<(), ()>
    {
        match expr {
            &SExpression::Scan { table, source_id, ref yield_fn } => {
                for row in self.storage.scan_table(table) {
                    let new_source = Source {
                        parent: source,
                        source_id: source_id,
                        row: &row
                    };

                    try!(self.execute(yield_fn, result_cb, Some(&new_source)));
                }

                Ok(())
            },
            &SExpression::Map { source_id, ref yield_in_fn, ref yield_out_fn } => {
                self.execute(yield_in_fn, &mut |row| {
                    let new_source = Source {
                        parent: source,
                        source_id: source_id,
                        row: row
                    };

                    self.execute(yield_out_fn, result_cb, Some(&new_source))
                }, source)
            },
            &SExpression::Yield { ref fields } => {
                let columns: Result<Vec<_>, ()>;
                columns = fields.iter().map(|e| self.resolve_value(e, source)).collect();
                match columns {
                    Ok(columns) => result_cb(&columns),
                    Err(()) => Err(())
                }
            },
            &SExpression::If { ref predicate, ref yield_fn } => {
                let pred_result = try!(self.resolve_value(predicate, source));

                if pred_result.tests_true() {
                    self.execute(yield_fn, result_cb, source)
                } else {
                    Ok(())
                }
            },
            &SExpression::ColumnField { .. } |
            &SExpression::BinaryOp { .. } |
            &SExpression::Value(..) => {
                // these expressions cannot contain yieldable rows.
                Err(())
            }
        }
    }

    fn resolve_value<'b>(&self, expr: &SExpression<'a, Storage::Info>,
        source: Option<&Source<'b, <Storage::Info as DatabaseInfo>::ColumnValue>>)
    -> Result<<Storage::Info as DatabaseInfo>::ColumnValue, ()>
    {
        match expr {
            &SExpression::Value(ref v) => Ok(v.clone()),
            &SExpression::ColumnField { source_id, column_offset } => {
                let row = source.and_then(|s| s.find_row_from_source_id(source_id));
                match row {
                    Some(row) => Ok(row[column_offset as usize].clone()),
                    None => Err(())
                }
            },
            &SExpression::BinaryOp { op, ref lhs, ref rhs } => {
                let l = try!(self.resolve_value(lhs, source));
                let r = try!(self.resolve_value(rhs, source));

                Ok(match op {
                    BinaryOp::Equal => l.equals(&r),
                    BinaryOp::NotEqual => l.not_equals(&r),
                    BinaryOp::And => l.and(&r),
                    BinaryOp::Or => l.or(&r),
                    BinaryOp::Concatenate => l.concat(&r),
                    _ => unimplemented!()
                })
            },
            &SExpression::Map { source_id, ref yield_in_fn, ref yield_out_fn } => {
                trace!("resolve_value; map {}", source_id);

                // yield_in_fn is expected to yield exactly one row
                // yield_out_fn is expected to return a single resolved value
                let mut r = None;
                let mut row_count = 0;

                try!(self.execute(yield_in_fn, &mut |row| {
                    if row_count == 0 {
                        r = Some(row.to_vec());
                    }
                    row_count += 1;
                    Ok(())
                }, source));

                if row_count == 1 {
                    let row = r.unwrap();

                    let new_source = Source {
                        parent: source,
                        source_id: source_id,
                        row: &row
                    };

                    self.resolve_value(yield_out_fn, Some(&new_source))
                } else {
                    Err(())
                }
            },
            _ => Err(())
        }
    }
}