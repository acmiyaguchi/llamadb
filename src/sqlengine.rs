use sqlsyntax::ast;
use types::Variant;

pub enum ExecuteStatementResponse<'a> {
    Created,
    Inserted(u64),
    Select {
        column_names: Box<[String]>,
        rows: Box<Iterator<Item=Box<[Variant]>> + 'a>
    },
    Explain(String)
}

pub type ExecuteStatementResult<'a> = Result<ExecuteStatementResponse<'a>, String>;

pub trait SqlEngine {
    fn new() -> Self;
    fn execute_statement(&mut self, stmt: ast::Statement) -> ExecuteStatementResult;
    fn create_table(&mut self, stmt: ast::CreateTableStatement) -> ExecuteStatementResult;
    fn insert_into(&mut self, stmt: ast::InsertStatement) -> ExecuteStatementResult;
    fn select(&self, stmt: ast::SelectStatement) -> ExecuteStatementResult;
    fn explain(&self, stmt: ast::ExplainStatement) -> ExecuteStatementResult;
}